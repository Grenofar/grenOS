#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

mod acpi;
mod anim;
mod desktop;
mod events;
mod fb;
mod font;
mod fs;
mod gdt;
mod heap;
mod icons;
mod idt;
mod keyboard;
mod memory;
mod mouse;
mod paging;
mod pci;
mod pic;
mod pit;
mod port;
mod power;
mod ps2;
mod rtc;
mod serial;
mod shell;
mod sysinfo;
mod time;
mod web;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::panic::PanicInfo;

use limine::BaseRevision;
use limine::request::{
    FramebufferRequest, HhdmRequest, MemoryMapRequest, RequestsEndMarker, RequestsStartMarker, RsdpRequest,
    StackSizeRequest,
};

#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[unsafe(link_section = ".requests")]
static STACK_SIZE_REQUEST: StackSizeRequest = StackSizeRequest::new().with_size(0x100000);

/// The screen: Limine sets a graphics mode and hands over its framebuffer.
#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

/// Which physical memory is RAM, and which is taken.
#[used]
#[unsafe(link_section = ".requests")]
static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

/// Where Limine maps all of physical memory in the higher half.
#[used]
#[unsafe(link_section = ".requests")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

/// The firmware's ACPI tables, for turning the machine off.
#[used]
#[unsafe(link_section = ".requests")]
static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();

#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();

#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

/// An address in a part of the higher half nothing maps: the paging check
/// puts a fresh page there.
const PROBE_PAGE: u64 = 0xFFFF_9000_0000_0000;

const MIB: u64 = 1024 * 1024;

/// The boot log: every line goes to the serial port, where the CI reads it,
/// and is kept for the terminal and the file system.
struct Log {
    lines: Vec<String>,
}

impl Log {
    fn say(&mut self, line: String) {
        serial::write_str(&line);
        serial::write_str("\n");
        self.lines.push(line);
    }
}

#[unsafe(no_mangle)]
extern "C" fn kmain() -> ! {
    assert!(BASE_REVISION.is_supported());

    serial::init();
    serial::write_str("grenOS\n");
    gdt::init();
    idt::init();

    // The IDT answers: the breakpoint handler prints its line and returns.
    // SAFETY: int3 only raises the breakpoint exception, which idt::init has
    // just given a handler.
    unsafe { core::arch::asm!("int3", options(nomem, nostack, preserves_flags)) };

    let Some(frame) = FRAMEBUFFER_REQUEST.get_response().and_then(|response| response.framebuffers().next()) else {
        serial::write_str("desktop: Limine gave no framebuffer\n");
        power::halt();
    };
    let mode = fb::Mode {
        width: frame.width() as usize,
        height: frame.height() as usize,
        pitch: frame.pitch() as usize,
        bits_per_pixel: usize::from(frame.bpp()),
        red_shift: frame.red_mask_shift(),
        green_shift: frame.green_mask_shift(),
        blue_shift: frame.blue_mask_shift(),
    };

    // Memory: the heap and the back buffer take the start of the largest
    // usable region, the frame allocator gets everything else.
    let (Some(map), Some(hhdm)) = (MEMORY_MAP_REQUEST.get_response(), HHDM_REQUEST.get_response()) else {
        serial::write_str("memory: Limine gave no memory map\n");
        power::halt();
    };
    let (entries, hhdm) = (map.entries(), hhdm.offset());
    let heap_size = heap::SIZE as u64;
    let back_size = (mode.width * mode.height * 4) as u64;
    let taken = (heap_size + back_size).div_ceil(memory::FRAME) * memory::FRAME;
    let Some(start) = memory::largest_usable(entries)
        .map(|(base, length)| (base.div_ceil(memory::FRAME) * memory::FRAME, base + length))
        .filter(|&(start, end)| start + taken <= end)
        .map(|(start, _)| start)
    else {
        serial::write_str("memory: no usable region holds the heap and the back buffer\n");
        power::halt();
    };
    // SAFETY: Limine's own memory map and HHDM offset; what the heap and the
    // back buffer take is kept out of the frame allocator.
    let mut frames = unsafe { memory::Frames::new(entries, hhdm, start..start + taken) };
    // SAFETY: that range is usable RAM, which the HHDM maps, given to the heap alone.
    unsafe { heap::init(start + hhdm) };

    // The heap works from here: the boot log can hold its own lines.
    let mut log = Log { lines: Vec::new() };
    log.lines.push("grenOS démarre".to_string());
    let total = (frames.total() * memory::FRAME + taken) / MIB;
    log.say(format!("memory: {} MiB usable, {} frames free", total, frames.free()));

    let before = frames.free();
    if let Some(frame) = frames.allocate() {
        let during = frames.free();
        // SAFETY: the frame has just been allocated, and nothing uses it.
        unsafe { frames.deallocate(frame) };
        log.say(format!("frames: {} free, {} with one taken, {} once given back", before, during, frames.free()));
    }

    let paging = check_paging(&mut frames);
    match paging {
        Ok(()) => log.say("paging: a fresh page mapped at 0xFFFF900000000000, written and read back".to_string()),
        Err(why) => log.say(format!("paging: {why}")),
    }
    let heap_line = {
        let numbers: Vec<u64> = (1..=1000).collect();
        let sum: u64 = numbers.iter().sum();
        format!("heap: a Vec of {} numbers sums to {}, {} bytes in use", numbers.len(), sum, heap::used())
    };
    log.say(heap_line);

    let devices = pci::scan();
    log.say(format!("pci: {} devices", devices.len()));
    let network = devices
        .iter()
        .find(|device| device.class == 0x02)
        .map(|device| format!("carte {} détectée, pilote à écrire", device.vendor_name()))
        .unwrap_or_else(|| "aucune carte réseau sur le bus".to_string());

    let acpi = RSDP_REQUEST
        .get_response()
        // SAFETY: the address Limine gives is the firmware's RSDP; base
        // revision 3 makes it physical, which is what `read` expects.
        .map(|response| unsafe { acpi::read(&mut frames, response.address() as u64) });
    let acpi = match acpi {
        Some(Ok(tables)) => {
            log.say(format!("acpi: {}", tables.summary()));
            Some(tables)
        }
        Some(Err(why)) => {
            log.say(format!("acpi: unreadable ({why})"));
            None
        }
        None => {
            log.say("acpi: Limine gave no RSDP".to_string());
            None
        }
    };
    let off_switch = acpi.as_ref().and_then(|tables| tables.power);

    pic::init();
    pit::init();
    let mouse_ok = ps2::init();
    // SAFETY: every vector the PIC can now raise has its handler in the IDT.
    unsafe { core::arch::asm!("sti", options(nomem, nostack)) };
    log.say(if mouse_ok { "input: keyboard and mouse".to_string() } else { "input: keyboard, no mouse".to_string() });

    font::tune(mode.height);
    // SAFETY: Limine maps the framebuffer it describes, height rows of pitch
    // bytes; the back buffer is the region reserved above, which the HHDM
    // maps and nothing else uses.
    let mut screen = unsafe { fb::Screen::new(frame.addr(), (start + heap_size + hhdm) as *mut u32, mode) };

    let machine = desktop::Machine {
        version: env!("CARGO_PKG_VERSION"),
        build: env!("GRENOS_BUILD"),
        built_at: env!("GRENOS_BUILT_AT"),
        cpu: sysinfo::cpu_name(),
        memory: (total, frames.free() * memory::FRAME / MIB),
        heap: (heap::used(), heap::SIZE),
        screen: format!("{} x {}, {} bits par point", mode.width, mode.height, mode.bits_per_pixel),
        paging: if paging.is_ok() { "quatre niveaux, page neuve vérifiée".to_string() } else { "en échec".to_string() },
        acpi: acpi.as_ref().map_or("absent".to_string(), |tables| tables.summary()),
        devices: devices.iter().map(|device| device.line()).collect(),
        log: log.lines.clone(),
        mouse: mouse_ok,
        can_power_off: off_switch.is_some(),
        network,
    };
    let files = files(&machine);
    let mut desk = desktop::Desktop::new(&screen, rtc::now(), machine, files, true);
    desk.frame(&mut screen);
    serial::write_str("desktop: drawn\n");

    let mut keyboard = keyboard::Keyboard::default();
    let mut decoder = mouse::Decoder::default();
    let mut packets: u32 = 0;
    let mut keys: u32 = 0;
    loop {
        while let Some(event) = events::pop() {
            let action = match event {
                events::Event::Second => {
                    desk.tick(rtc::now(), gauge(packets, keys));
                    None
                }
                events::Event::Key(code) => keyboard.feed(code).and_then(|key| {
                    keys += 1;
                    if keys == 1 {
                        serial::write_str("input: first key decoded\n");
                    }
                    desk.key(key)
                }),
                events::Event::Mouse(byte) => decoder.feed(byte).and_then(|packet| {
                    packets += 1;
                    if packets == 1 {
                        let (x, y) = desk.pointer();
                        serial::write_str(&format!("input: first mouse packet decoded, pointer at {x} {y}\n"));
                    }
                    desk.mouse(packet)
                }),
            };
            match action {
                Some(desktop::Action::Reboot) => {
                    desk.frame(&mut screen);
                    power::reboot();
                }
                Some(desktop::Action::PowerOff) => {
                    // The goodbye screen goes up first, and stays up if the
                    // firmware ignores us.
                    desk.frame(&mut screen);
                    match off_switch {
                        Some(switch) => power::off(switch),
                        None => power::halt(),
                    }
                }
                Some(desktop::Action::Lock) | None => {}
            }
        }
        // Everything that moves is moved by the clock, then drawn once.
        desk.advance(events::millis());
        desk.frame(&mut screen);
        idle();
    }
}

/// What the file explorer finds at boot: the machine talking about itself.
fn files(machine: &desktop::Machine) -> fs::Fs {
    let mut files = fs::Fs::new();
    files.add_system("/Système/demarrage.txt", &machine.log.join("\n"));
    files.add_system("/Système/materiel.txt", &machine.devices.join("\n"));
    files.add_system(
        "/Système/version.txt",
        &format!(
            "grenOS {}\nbuild {}\ncompilée le {}\n{}\n",
            machine.version,
            machine.build,
            machine.built_at,
            machine.lines().join("\n")
        ),
    );
    files.add_system(
        "/Documents/lisez-moi.txt",
        "Ce dossier vit en mémoire.\n\nLe bloc-notes enregistre ici : bouton Enregistrer.\nTout disparaît à l'extinction, tant que le pilote de disque n'existe pas.\n",
    );
    files
}

/// What Paramètres shows about the input devices.
fn gauge(packets: u32, keys: u32) -> desktop::Input {
    let counts = ps2::counts();
    desktop::Input {
        mouse_bytes: counts.mouse_bytes,
        key_bytes: counts.key_bytes,
        mouse_irq: counts.mouse_irq,
        key_irq: counts.key_irq,
        swept: counts.swept,
        lost: events::lost(),
        packets,
        keys,
        uptime: (events::ticks_seconds()) as u32,
    }
}

/// Maps a fresh frame at PROBE_PAGE, writes through the new mapping, and
/// reads the same bytes back through the HHDM.
fn check_paging(frames: &mut memory::Frames) -> Result<(), &'static str> {
    let frame = frames.allocate().ok_or("no frame left")?;
    // SAFETY: nothing maps PROBE_PAGE, and the frame has just been allocated.
    unsafe { paging::map(frames, PROBE_PAGE, frame)? };
    let marker = u64::from_le_bytes(*b"grenOS!\0");
    // SAFETY: PROBE_PAGE is now mapped, present and writable, to `frame`,
    // which the HHDM maps too.
    let (seen, direct) = unsafe {
        let page = PROBE_PAGE as *mut u64;
        page.write_volatile(marker);
        (page.read_volatile(), ((frame + frames.hhdm()) as *const u64).read_volatile())
    };
    if seen == marker && direct == marker {
        Ok(())
    } else {
        Err("the new page and its frame disagree")
    }
}

/// Waits for the next interrupt, unless one has already left an event. The
/// timer wakes the machine a thousand times a second, so an animation still
/// advances while nothing else happens.
fn idle() {
    // SAFETY: cli, check, then sti immediately followed by hlt: sti takes
    // effect after the next instruction, so an interrupt arriving after the
    // check still wakes the hlt, and no event waits for the one after it.
    unsafe {
        core::arch::asm!("cli", options(nomem, nostack));
        if events::is_empty() {
            core::arch::asm!("sti", "hlt", options(nomem, nostack));
        } else {
            core::arch::asm!("sti", options(nomem, nostack));
        }
    }
}

#[panic_handler]
fn rust_panic(_info: &PanicInfo) -> ! {
    serial::write_str("panic\n");
    power::halt()
}

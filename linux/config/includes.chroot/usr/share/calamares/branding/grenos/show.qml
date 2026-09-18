/* Ce que grenOS raconte pendant qu'il s'installe.
 *
 * Trois écrans, en français, sans image à charger : du texte sur le fond
 * sombre du bureau. Une installation qui ne dit rien paraît toujours plus
 * longue qu'elle ne l'est.
 */
import QtQuick 2.5
import calamares.slideshow 1.0

Presentation {
    id: presentation

    function nextSlide() {
        presentation.goToNextSlide();
    }

    Timer {
        id: advanceTimer
        interval: 12000
        running: presentation.activatedInCalamares
        repeat: true
        onTriggered: nextSlide()
    }

    Slide {
        Rectangle {
            anchors.fill: parent
            color: "#0b1120"
            Column {
                anchors.centerIn: parent
                spacing: 18
                width: parent.width * 0.8
                Text {
                    text: "grenOS s'installe"
                    color: "#ffffff"
                    font.pixelSize: 34
                    font.bold: true
                    horizontalAlignment: Text.AlignHCenter
                    width: parent.width
                }
                Text {
                    text: "Un système complet, construit sur Debian : le bureau, les applications, et les mises à jour de sécurité qui suivent."
                    color: "#9aa7bd"
                    font.pixelSize: 18
                    wrapMode: Text.WordWrap
                    horizontalAlignment: Text.AlignHCenter
                    width: parent.width
                }
            }
        }
    }

    Slide {
        Rectangle {
            anchors.fill: parent
            color: "#0b1120"
            Column {
                anchors.centerIn: parent
                spacing: 18
                width: parent.width * 0.8
                Text {
                    text: "Un magasin, et Steam"
                    color: "#ffffff"
                    font.pixelSize: 34
                    font.bold: true
                    horizontalAlignment: Text.AlignHCenter
                    width: parent.width
                }
                Text {
                    text: "Le magasin installe les applications en un clic, paquets Debian comme Flatpak. Steam s'y trouve, et les pilotes qui vont avec."
                    color: "#9aa7bd"
                    font.pixelSize: 18
                    wrapMode: Text.WordWrap
                    horizontalAlignment: Text.AlignHCenter
                    width: parent.width
                }
            }
        }
    }

    Slide {
        Rectangle {
            anchors.fill: parent
            color: "#0b1120"
            Column {
                anchors.centerIn: parent
                spacing: 18
                width: parent.width * 0.8
                Text {
                    text: "Les mises à jour viennent à vous"
                    color: "#ffffff"
                    font.pixelSize: 34
                    font.bold: true
                    horizontalAlignment: Text.AlignHCenter
                    width: parent.width
                }
                Text {
                    text: "« Mise à jour de grenOS » apporte les nouveautés du système et les correctifs de sécurité de Debian. Plus jamais d'image à retélécharger."
                    color: "#9aa7bd"
                    font.pixelSize: 18
                    wrapMode: Text.WordWrap
                    horizontalAlignment: Text.AlignHCenter
                    width: parent.width
                }
            }
        }
    }

    Component.onCompleted: presentation.currentSlide = 0
}

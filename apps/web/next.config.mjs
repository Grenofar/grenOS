/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  // Le site ne fait que lire Supabase et créer des missions.
  // Aucun agent ne s'exécute ici : les fonctions serverless sont coupées après
  // quelques minutes, ce qui est incompatible avec du travail de fond (D-001).
};

export default nextConfig;

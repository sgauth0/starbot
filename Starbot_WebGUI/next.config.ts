import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  /* config options here */
  reactCompiler: true,
  output: 'standalone', // Enable standalone output for production deployment
  turbopack: {
    // Prevent Next.js from inferring /var/www/sites/stella/starbot.cloud as workspace root
    // when both root and app lockfiles exist during deploy.
    root: process.cwd(),
  },
};

export default nextConfig;

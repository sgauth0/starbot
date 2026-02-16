FROM node:22-alpine

WORKDIR /app

# Copy standalone build
COPY .next/standalone ./
COPY .next/static ./.next/static
COPY public ./public

# Set environment variables
ENV NODE_ENV=production
ENV PORT=3000
ENV HOSTNAME="0.0.0.0"

# Expose web port
EXPOSE 3000

# Run Next.js
CMD ["node", "server.js"]

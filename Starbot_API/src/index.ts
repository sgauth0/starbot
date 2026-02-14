// Starbot API - The Brain
// Port: 3737 (localhost only)

// Load environment variables from .env file
import 'dotenv/config';

import Fastify from 'fastify';
import cors from '@fastify/cors';
import websocket from '@fastify/websocket';
import { env, logConfiguration } from './env.js';
import { projectRoutes } from './routes/projects.js';
import { chatRoutes } from './routes/chats.js';
import { messageRoutes } from './routes/messages.js';
import { generationRoutes } from './routes/generation.js';
import { modelRoutes } from './routes/models.js';
import { workspaceRoutes } from './routes/workspaces.js';
import { memoryRoutes } from './routes/memory.js';
import { authRoutes } from './routes/auth.js';

const PORT = env.PORT;
const HOST = env.HOST;

const server = Fastify({
  logger: {
    level: process.env.LOG_LEVEL || 'info',
    transport: {
      target: 'pino-pretty',
      options: {
        translateTime: 'HH:MM:ss Z',
        ignore: 'pid,hostname',
      },
    },
  },
});

// CORS for local development
await server.register(cors, {
  origin: [
    'http://localhost:8080',
    'http://127.0.0.1:8080',
    'http://localhost:3000',      // WebGUI
    'http://127.0.0.1:3000'
  ],
  credentials: true,
});

// WebSocket support for streaming
await server.register(websocket);

// Main health endpoint with /v1 prefix
server.get('/v1/health', async () => {
  return {
    status: 'ok',
    timestamp: new Date().toISOString(),
    version: '1.0.0',
  };
});

// Legacy redirect
server.get('/health', async (request, reply) => {
  return reply.redirect(301, '/v1/health');
});

// API routes
await server.register(projectRoutes, { prefix: '/v1' });
await server.register(chatRoutes, { prefix: '/v1' });
await server.register(messageRoutes, { prefix: '/v1' });
await server.register(generationRoutes, { prefix: '/v1' });
await server.register(modelRoutes, { prefix: '/v1' });
await server.register(workspaceRoutes, { prefix: '/v1' });
await server.register(memoryRoutes, { prefix: '/v1' });
await server.register(authRoutes, { prefix: '/v1' });

// Start server
try {
  await server.listen({ port: PORT, host: HOST });
  console.log(`🧠 Starbot API listening on http://${HOST}:${PORT}`);
  console.log(`📊 Health: http://${HOST}:${PORT}/health`);
  console.log('');
  logConfiguration();
} catch (err) {
  server.log.error(err);
  process.exit(1);
}

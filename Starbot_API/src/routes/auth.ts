import { FastifyPluginAsync } from 'fastify';
import crypto from 'crypto';

// In-memory storage for device codes (STUB - use Redis in production)
interface DeviceAuthRequest {
  device_code: string;
  user_code: string;
  verification_url: string;
  expires_at: number;
  status: 'pending' | 'authorized' | 'denied' | 'expired';
  access_token?: string;
}

const deviceRequests = new Map<string, DeviceAuthRequest>();

// Helper to generate random codes
function generateCode(length: number): string {
  return crypto.randomBytes(length).toString('hex').slice(0, length).toUpperCase();
}

// Helper to generate user-friendly code
function generateUserCode(): string {
  const chars = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789'; // Exclude confusing chars
  let code = '';
  for (let i = 0; i < 6; i++) {
    code += chars[Math.floor(Math.random() * chars.length)];
  }
  // Format as XXX-XXX
  return `${code.slice(0, 3)}-${code.slice(3)}`;
}

// Cleanup expired requests every minute
setInterval(() => {
  const now = Date.now();
  for (const [device_code, request] of deviceRequests.entries()) {
    if (request.expires_at < now) {
      request.status = 'expired';
    }
  }
}, 60000);

export const authRoutes: FastifyPluginAsync = async (server) => {
  // Start device authorization flow
  server.post('/auth/device/start', async (request, reply) => {
    const device_code = generateCode(32);
    const user_code = generateUserCode();
    const expires_at = Date.now() + 15 * 60 * 1000; // 15 minutes

    const authRequest: DeviceAuthRequest = {
      device_code,
      user_code,
      verification_url: 'http://localhost:3000/auth/device',
      expires_at,
      status: 'pending',
    };

    deviceRequests.set(device_code, authRequest);

    return {
      device_code,
      user_code,
      verification_url: authRequest.verification_url,
      expires_in: 900, // 15 minutes in seconds
      interval: 5, // Poll every 5 seconds
    };
  });

  // Poll device authorization status
  server.post('/auth/device/poll', async (request, reply) => {
    const { device_code } = request.body as { device_code: string };

    const authRequest = deviceRequests.get(device_code);

    if (!authRequest) {
      return reply.status(404).send({ error: 'Device code not found' });
    }

    if (authRequest.expires_at < Date.now()) {
      authRequest.status = 'expired';
    }

    if (authRequest.status === 'authorized') {
      return {
        status: 'authorized',
        access_token: authRequest.access_token,
      };
    }

    if (authRequest.status === 'denied') {
      return reply.status(403).send({
        error: 'authorization_denied',
        message: 'User denied the authorization request',
      });
    }

    if (authRequest.status === 'expired') {
      return reply.status(410).send({
        error: 'expired_token',
        message: 'Device code has expired',
      });
    }

    return {
      status: 'pending',
      message: 'User has not yet authorized this device',
    };
  });

  // Confirm device authorization (called by WebGUI)
  server.post('/auth/device/confirm', async (request, reply) => {
    const { user_code, action } = request.body as {
      user_code: string;
      action?: 'approve' | 'deny';
    };

    // Find the request with this user code
    let authRequest: DeviceAuthRequest | undefined;
    for (const request of deviceRequests.values()) {
      if (request.user_code === user_code) {
        authRequest = request;
        break;
      }
    }

    if (!authRequest) {
      return reply.status(404).send({ error: 'User code not found' });
    }

    if (authRequest.expires_at < Date.now()) {
      authRequest.status = 'expired';
      return reply.status(410).send({ error: 'Code has expired' });
    }

    if (action === 'deny') {
      authRequest.status = 'denied';
      return { status: 'denied' };
    }

    // Generate access token (simple random token for now)
    const access_token = crypto.randomBytes(32).toString('hex');

    authRequest.status = 'authorized';
    authRequest.access_token = access_token;

    return {
      status: 'authorized',
      message: 'Device authorized successfully',
    };
  });

  // Get pending authorization request (for WebGUI to display)
  server.get('/auth/device/pending/:user_code', async (request, reply) => {
    const { user_code } = request.params as { user_code: string };

    // Find the request with this user code
    let authRequest: DeviceAuthRequest | undefined;
    for (const request of deviceRequests.values()) {
      if (request.user_code === user_code) {
        authRequest = request;
        break;
      }
    }

    if (!authRequest) {
      return reply.status(404).send({ error: 'User code not found' });
    }

    if (authRequest.expires_at < Date.now()) {
      authRequest.status = 'expired';
      return reply.status(410).send({ error: 'Code has expired' });
    }

    return {
      user_code: authRequest.user_code,
      status: authRequest.status,
      expires_in: Math.floor((authRequest.expires_at - Date.now()) / 1000),
    };
  });
};

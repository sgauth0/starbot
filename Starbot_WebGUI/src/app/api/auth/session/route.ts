import { NextResponse } from 'next/server';
import { z } from 'zod';
import {
  evaluateAdmin,
  getAdminCookieName,
  issueAdminCookieToken,
} from '@/lib/server/admin-cookie';

const SessionBodySchema = z.object({
  email: z.string().email(),
  name: z.string().min(1),
  adminCode: z.string().optional(),
});

const ADMIN_COOKIE_NAME = getAdminCookieName();

export async function POST(request: Request) {
  let body: z.infer<typeof SessionBodySchema>;
  try {
    body = SessionBodySchema.parse(await request.json());
  } catch {
    return NextResponse.json({ error: 'Invalid session payload' }, { status: 400 });
  }

  const isAdmin = evaluateAdmin(body.email, body.adminCode);
  const role = isAdmin ? 'admin' : 'user';
  const response = NextResponse.json({ role });

  if (isAdmin) {
    const token = await issueAdminCookieToken(body.email);
    if (token) {
      response.cookies.set({
        name: ADMIN_COOKIE_NAME,
        value: token,
        httpOnly: true,
        secure: process.env.NODE_ENV === 'production',
        sameSite: 'lax',
        path: '/',
        maxAge: 60 * 60 * 8,
      });
    }
  } else {
    response.cookies.delete(ADMIN_COOKIE_NAME);
  }

  return response;
}

export async function DELETE() {
  const response = NextResponse.json({ ok: true });
  response.cookies.delete(ADMIN_COOKIE_NAME);
  return response;
}

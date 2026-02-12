import { API_BASE_URL, API_TOKEN_HEADER, getApiToken } from './config';
import { z } from 'zod';

export class ApiError extends Error {
  constructor(public status: number, public message: string, public data?: any) {
    super(message);
    this.name = 'ApiError';
  }
}

async function fetchApi<T>(path: string, options: RequestInit = {}, schema?: z.ZodSchema<T>): Promise<T> {
  const token = getApiToken();
  const headers = new Headers(options.headers);

  if (token) {
    headers.set(API_TOKEN_HEADER, token);
  }

  if (!headers.has('Content-Type') && !(options.body instanceof FormData)) {
    headers.set('Content-Type', 'application/json');
  }

  const url = `${API_BASE_URL}${path}`;
  const response = await fetch(url, { ...options, headers });

  if (!response.ok) {
    let errorMessage = `API Error: ${response.status} ${response.statusText}`;
    let errorData;
    try {
      errorData = await response.json();
      if (errorData && errorData.message) {
        errorMessage = errorData.message;
      }
    } catch (e) {
      // ignore
    }
    throw new ApiError(response.status, errorMessage, errorData);
  }

  if (response.status === 204) {
    return {} as T;
  }

  let data;
  try {
    data = await response.json();
  } catch (e) {
      throw new ApiError(response.status, 'Invalid JSON response');
  }

  if (schema) {
      const result = schema.safeParse(data);
      if (!result.success) {
          console.error('Validation Error:', result.error);
          throw new ApiError(response.status, 'Response validation failed');
      }
      return result.data;
  }

  return data as T;
}

export const api = {
  get: <T>(path: string, schema?: z.ZodSchema<T>) => fetchApi<T>(path, { method: 'GET' }, schema),
  post: <T>(path: string, body: any, schema?: z.ZodSchema<T>) => fetchApi<T>(path, { method: 'POST', body: JSON.stringify(body) }, schema),
  put: <T>(path: string, body: any, schema?: z.ZodSchema<T>) => fetchApi<T>(path, { method: 'PUT', body: JSON.stringify(body) }, schema),
  delete: <T>(path: string) => fetchApi<T>(path, { method: 'DELETE' }),
  patch: <T>(path: string, body: any, schema?: z.ZodSchema<T>) => fetchApi<T>(path, { method: 'PATCH', body: JSON.stringify(body) }, schema),
};

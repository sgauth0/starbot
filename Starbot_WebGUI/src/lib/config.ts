export const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:3737/v1';
export const API_TOKEN_HEADER = 'X-API-Token';

export const getApiToken = () => {
  if (typeof window !== 'undefined') {
    return localStorage.getItem('starbot_api_token') || '';
  }
  return '';
};

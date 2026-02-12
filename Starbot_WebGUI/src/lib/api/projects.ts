import { api } from '../api';
import { Project, ProjectSchema } from '../types';
import { z } from 'zod';

export const projectsApi = {
  list: () => api.get<Project[]>('/projects', z.array(ProjectSchema)),
  get: (id: string) => api.get<Project>(`/projects/${id}`, ProjectSchema),
  create: (data: Partial<Project>) => api.post<Project>('/projects', data, ProjectSchema),
  update: (id: string, data: Partial<Project>) => api.put<Project>(`/projects/${id}`, data, ProjectSchema),
  delete: (id: string) => api.delete<void>(`/projects/${id}`),
};

import { api } from '../api';
import { Project, ProjectSchema } from '../types';
import { z } from 'zod';

// API response wrapper schemas
const ProjectsResponseSchema = z.object({
  projects: z.array(ProjectSchema),
});

const ProjectResponseSchema = z.object({
  project: ProjectSchema,
});

export const projectsApi = {
  list: async () => {
    const response = await api.get('/projects', ProjectsResponseSchema);
    return response.projects;
  },

  get: async (id: string) => {
    const response = await api.get(`/projects/${id}`, ProjectResponseSchema);
    return response.project;
  },

  create: async (data: Partial<Project>) => {
    const response = await api.post('/projects', data, ProjectResponseSchema);
    return response.project;
  },

  update: async (id: string, data: Partial<Project>) => {
    const response = await api.put(`/projects/${id}`, data, ProjectResponseSchema);
    return response.project;
  },

  delete: (id: string) => api.delete<void>(`/projects/${id}`),
};

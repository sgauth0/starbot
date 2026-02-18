// Tool System Initialization
// Registers all available tools on startup

import { env } from '../../env.js';
import { toolRegistry } from './registry.js';
import { webSearchTool } from './web-search-tool.js';
import { calculatorTool } from './calculator-tool.js';
import { codeExecTool } from './code-exec-tool.js';
import { fileReadTool } from './file-read-tool.js';

export { toolRegistry } from './registry.js';
export type { ToolDefinition, ToolResult, ToolCall, ToolParameter } from './types.js';

export function initializeTools(): void {
  console.log('Initializing tool system...');

  // Register web search tool if enabled
  if (env.TOOLS_ENABLED && env.WEB_SEARCH_ENABLED && env.BRAVE_SEARCH_API_KEY) {
    toolRegistry.register(webSearchTool);
    console.log('✓ Web search tool registered');
  }

  // Register calculator tool
  if (env.TOOLS_ENABLED) {
    toolRegistry.register(calculatorTool);
    console.log('✓ Calculator tool registered');
  }

  // Register code execution tool (disabled by default for security)
  if (env.TOOLS_ENABLED && env.CODE_EXECUTION_ENABLED) {
    toolRegistry.register(codeExecTool);
    console.log('✓ Code execution tool registered (SECURITY WARNING: Code execution enabled)');
  }

  // Register file read tool
  if (env.TOOLS_ENABLED) {
    toolRegistry.register(fileReadTool);
    console.log('✓ File read tool registered');
  }

  const registeredTools = toolRegistry.getAll();
  console.log(`Tool system initialized with ${registeredTools.length} tool(s)`);

  if (registeredTools.length > 0) {
    console.log(`Registered tools: ${registeredTools.map(t => t.name).join(', ')}`);
  }
}

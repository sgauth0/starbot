// Quick test for triage system
import { runTriage } from './src/services/triage/index.js';

const tests = [
  {
    name: 'Simple chat',
    input: { user_message: 'Hello, how are you?' },
    expectedCategory: 'CHAT_QA',
    expectedLane: 'quick',
  },
  {
    name: 'Debug request',
    input: { user_message: 'Debug this error:\n\nTraceback (most recent call last):\n  File "app.py", line 10\n    return foo()\nNameError: name \'foo\' is not defined' },
    expectedCategory: 'DEBUG',
    expectedLane: 'standard',
  },
  {
    name: 'Code implementation',
    input: { user_message: 'Implement a function that checks if a number is prime' },
    expectedCategory: 'CODE_CHANGE',
    expectedLane: 'standard',
  },
  {
    name: 'Code explanation',
    input: { user_message: 'Explain what this code does:\n\n```python\ndef fib(n):\n  if n <= 1: return n\n  return fib(n-1) + fib(n-2)\n```' },
    expectedCategory: 'CODE_EXPLAIN',
    expectedLane: 'quick',
  },
  {
    name: 'Summary request',
    input: { user_message: 'Summarize this article about AI...' },
    expectedCategory: 'SUMMARIZE',
    expectedLane: 'quick',
  },
  {
    name: 'Deep analysis',
    input: { user_message: 'Give me a thorough, detailed, in-depth analysis of microservices architecture patterns' },
    expectedCategory: 'PLAN_DESIGN',
    expectedLane: 'deep',
  },
  {
    name: 'Quick question',
    input: { user_message: 'Quick: what is REST?', mode: 'quick' as const },
    expectedCategory: 'CHAT_QA',
    expectedLane: 'quick',
  },
];

console.log('🧪 Triage System Tests\n');

let passed = 0;
let failed = 0;

for (const test of tests) {
  const result = runTriage(test.input);
  const categoryMatch = result.decision.category === test.expectedCategory;
  const laneMatch = result.decision.lane === test.expectedLane;
  const success = categoryMatch && laneMatch;

  if (success) {
    console.log(`✅ ${test.name}`);
    passed++;
  } else {
    console.log(`❌ ${test.name}`);
    console.log(`   Expected: ${test.expectedCategory}/${test.expectedLane}`);
    console.log(`   Got:      ${result.decision.category}/${result.decision.lane}`);
    console.log(`   Reasons:  ${result.decision.reason_codes.slice(0, 3).join(', ')}`);
    failed++;
  }
}

console.log(`\n📊 Results: ${passed} passed, ${failed} failed`);
console.log(`⏱️  Average triage time: ${tests.map(t => runTriage(t.input).elapsed_ms).reduce((a, b) => a + b, 0) / tests.length}ms`);

process.exit(failed > 0 ? 1 : 0);

import { config } from 'dotenv';
import { spawn } from 'child_process';
import path from 'path';

// Load env variables
config();

// Get command line arguments
const args = process.argv.slice(2);
if (args.length === 0) {
  console.error('No command specified');
  process.exit(1);
}

// Execute the specified command with environment variables from .env
const command = args[0];
const commandArgs = args.slice(1);

const child = spawn(command, commandArgs, {
  stdio: 'inherit',
  shell: true,
  env: { ...process.env } // Pass all environment variables including those from .env
});

child.on('close', (code) => {
  process.exit(code);
});
// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

// This script generates a JSON file based on the environment variables
// Sample environment variables:
// AUTHENTICATION_PROVIDER_TYPE=generix
// API_BASE_URL=https://localhost:8080
// GENERIX_AUTHORITY=https://localhost:8080
// GENERIX_CLIENT_ID=LaserSmartClient
// GENERIX_REDIRECT_URI=https://localhost:8080/client/
// GENERIX_SCOPE=openid email profile read:api write:api
// GENERIX_AUDIENCE=LaserSmart
// GENERIX_TOKEN_ISSUER=https://localhost:8080
// GENERIX_JWKS_ENDPOINT=https://localhost:8080/.well-known/jwks.json
// GENERIX_DOMAIN=localhost
// Sample output:
// {
//   "provider": "generix",
//   "api_base_url": "https://localhost:8080",
//   "authority": "https://localhost:8080",
//   "client_id": "LaserSmartClient",
//   "scope": "openid email profile read:api write:api",
//   "redirect_uri": "https://localhost:8080/client/",
//   "audience": "LaserSmart",
//   "token_issuer": "https://localhost:8080",
//   "jwks_endpoint": "https://localhost:8080/.well-known/jwks.json",
//   "domain": "localhost"
// }

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import dotenv from "dotenv";
dotenv.config();
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const outputFilePath = path.join(__dirname, "public", "generix.json");
const config = {
  provider: process.env.AUTHENTICATION_PROVIDER_TYPE || "generix",
  api_base_url: process.env.API_BASE_URL || "https://localhost:8080",
  authority: process.env.GENERIX_AUTHORITY || "https://localhost:8080",
  client_id: process.env.GENERIX_CLIENT_ID || "LaserSmartClient",
  scope: process.env.GENERIX_SCOPE || "openid email profile read:api write:api",
  redirect_uri:
    process.env.GENERIX_REDIRECT_URI || "https://localhost:8080/client/",
  audience: process.env.GENERIX_AUDIENCE || "LaserSmart",
  token_issuer: process.env.GENERIX_TOKEN_ISSUER || "https://localhost:8080",
  jwks_endpoint:
    process.env.GENERIX_JWKS_ENDPOINT ||
    "https://localhost:8080/.well-known/jwks.json",
  domain: process.env.GENERIX_DOMAIN || "localhost",
};
fs.writeFileSync(outputFilePath, JSON.stringify(config, null, 2), "utf8");
console.log(`Configuration file generated at ${outputFilePath}`);
// To run this script, use the command: node generate.generix.json.mjs

// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

/**
 * @fileoverview Utilities for extracting base64 encoded PEM certificates and keys from YAML configuration files.
 *
 * This module provides functions to read and decode SSL/TLS certificates and private keys
 * that are stored as base64-encoded strings in YAML configuration files.
 */

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

import yaml from "js-yaml";

/**
 * Configuration object structure for type safety
 */
interface VisualizationConfig {
  visualization?: {
    cert?: string;
    key?: string;
  };
}

/**
 * Generic function to extract and decode base64-encoded data from a YAML configuration file.
 *
 * @param configFile - Relative path to the YAML configuration file
 * @param fieldPath - Dot-separated path to the field containing the base64 data (e.g., 'visualization.cert')
 * @param fieldName - Human-readable name of the field for error messages
 * @returns The decoded UTF-8 string content
 *
 * @throws {Error} When the configuration file cannot be read, parsed, or the specified field is missing
 *
 * @example
 * ```typescript
 * const cert = extractBase64Field('../config.yaml', 'visualization.cert', 'Certificate');
 * const key = extractBase64Field('../config.yaml', 'visualization.key', 'Private key');
 * ```
 */
function extractBase64Field(
  configFile: string,
  fieldPath: string,
  fieldName: string,
): string {
  const __filename = fileURLToPath(import.meta.url);
  const __dirname = path.dirname(__filename);
  const configPath = path.join(__dirname, configFile);

  try {
    const configContent = fs.readFileSync(configPath, "utf8");
    const config: VisualizationConfig = yaml.load(
      configContent,
    ) as VisualizationConfig;

    // Navigate to the nested field using the dot-separated path
    const pathParts = fieldPath.split(".");
    let currentObj: any = config;

    for (const part of pathParts) {
      if (
        !currentObj ||
        typeof currentObj !== "object" ||
        !(part in currentObj)
      ) {
        throw new Error(
          `${fieldName} not found in configuration file at path: ${fieldPath}`,
        );
      }
      currentObj = currentObj[part];
    }

    if (typeof currentObj !== "string") {
      throw new Error(`${fieldName} must be a string value`);
    }

    // Decode the base64 data
    const dataBuffer = Buffer.from(currentObj, "base64");

    return dataBuffer.toString("utf8");
  } catch (error) {
    if (error instanceof Error) {
      console.error(
        `Error extracting ${fieldName.toLowerCase()}:`,
        error.message,
      );
    } else {
      console.error(`Error extracting ${fieldName.toLowerCase()}:`, error);
    }
    throw error;
  }
}

/**
 * Extracts and decodes a base64-encoded PEM certificate from a YAML configuration file.
 *
 * The function reads the specified configuration file and extracts the certificate
 * from the `visualization.cert` field, then decodes it from base64 to UTF-8.
 *
 * @param configFile - Relative path to the YAML configuration file (e.g., '../config.yaml')
 * @returns The decoded PEM certificate as a UTF-8 string
 *
 * @throws {Error} When the configuration file cannot be read, parsed, or the certificate field is missing
 *
 * @example
 * ```typescript
 * try {
 *   const certificate = extractCert('../config.yaml');
 *   console.log('Certificate extracted successfully');
 * } catch (error) {
 *   console.error('Failed to extract certificate:', error.message);
 * }
 * ```
 */
export function extractCert(configFile: string): string {
  return extractBase64Field(configFile, "visualization.cert", "Certificate");
}

/**
 * Extracts and decodes a base64-encoded private key from a YAML configuration file.
 *
 * The function reads the specified configuration file and extracts the private key
 * from the `visualization.key` field, then decodes it from base64 to UTF-8.
 *
 * @param configFile - Relative path to the YAML configuration file (e.g., '../config.yaml')
 * @returns The decoded private key as a UTF-8 string
 *
 * @throws {Error} When the configuration file cannot be read, parsed, or the key field is missing
 *
 * @example
 * ```typescript
 * try {
 *   const privateKey = extractKey('../config.yaml');
 *   console.log('Private key extracted successfully');
 * } catch (error) {
 *   console.error('Failed to extract private key:', error.message);
 * }
 * ```
 */
export function extractKey(configFile: string): string {
  return extractBase64Field(configFile, "visualization.key", "Private key");
}

/**
 * Extracts both certificate and private key from a YAML configuration file.
 *
 * This is a convenience function that extracts both the certificate and private key
 * in a single operation, useful when both are needed together for SSL/TLS setup.
 *
 * @param configFile - Relative path to the YAML configuration file (e.g., '../config.yaml')
 * @returns An object containing both the certificate and private key as UTF-8 strings
 *
 * @throws {Error} When the configuration file cannot be read, parsed, or either field is missing
 *
 * @example
 * ```typescript
 * try {
 *   const { cert, key } = extractCertAndKey('../config.yaml');
 *   // Use cert and key for HTTPS server setup
 * } catch (error) {
 *   console.error('Failed to extract certificate and key:', error.message);
 * }
 * ```
 */
export function extractCertAndKey(configFile: string): {
  cert: string;
  key: string;
} {
  return {
    cert: extractCert(configFile),
    key: extractKey(configFile),
  };
}

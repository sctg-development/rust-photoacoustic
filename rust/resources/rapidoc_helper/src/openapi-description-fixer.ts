/**
 * OpenAPI Description Fixer
 * 
 * Fixes common escaping issues in OpenAPI descriptions that prevent markdown rendering.
 * Handles escaped newlines, tabs, and other whitespace issues.
 * 
 * @module openapi-description-fixer
 * @version 1.0.0
 */

import type { paths, components } from 'openapi3';

/**
 * OpenAPI 3.0 Specification Type Definition
 * 
 * Represents the structure of an OpenAPI/Swagger specification document.
 * This interface defines the required and optional fields found in OpenAPI 3.0 documents.
 * 
 * @typedef {Object} OpenAPI3
 * @property {string} openapi - OpenAPI specification version (e.g., "3.0.0")
 * @property {Object} info - API metadata
 * @property {string} info.title - Human-readable API title/name
 * @property {string} info.version - API version number
 * @property {paths} paths - All available API endpoints and their operations
 * @property {components} components - Reusable API components (schemas, security schemes, etc.)
 * @property {string} [host] - API server host (optional, for Swagger 2.0 compatibility)
 * @property {string} [basePath] - Base path for all API endpoints (optional)
 */
type OpenAPI3 = {
    openapi: string;
    info: {
        title: string;
        version: string;
    };
    paths: paths;
    components: components;
    host?: string;
    basePath?: string;
}

/**
 * OpenAPI Operation object structure
 */
interface OpenAPIOperation {
    description?: string;
    [key: string]: any;
}

/**
 * OpenAPI Path Item object structure
 */
interface OpenAPIPathItem {
    get?: OpenAPIOperation;
    post?: OpenAPIOperation;
    put?: OpenAPIOperation;
    delete?: OpenAPIOperation;
    patch?: OpenAPIOperation;
    options?: OpenAPIOperation;
    head?: OpenAPIOperation;
    trace?: OpenAPIOperation;
    [key: string]: any;
}

/**
 * Format JSON string by adding newlines and indentation
 * Simulates JSON.stringify(obj, null, 2) formatting via text replacement
 * 
 * @param {string} jsonString - The JSON string to format
 * @returns {string} The formatted JSON string
 */
function formatJsonString(jsonString: string): string {
    return jsonString
        // Add newline after opening braces and brackets
        .replace(/{/g, '{\n  ')
        .replace(/\[/g, '[\n  ')
        // Add newline before closing braces and brackets
        .replace(/}/g, '\n}')
        .replace(/]/g, '\n]')
        // Add newline after commas and indent next item
        .replace(/,/g, ',\n  ')
        // Add newline after colons in key-value pairs and a space
        .replace(/:/g, ': ')
        // Clean up multiple spaces/newlines
        .replace(/\n\s+/g, '\n  ')
        // Remove trailing spaces on lines
        .replace(/ +$/gm, '');
}

/**
 * Fix escaped newlines and whitespace in a description string
 * 
 * Converts literal escape sequences like `\n` and `\t` to actual newlines and tabs.
 * Also fixes common formatting issues in markdown like:
 * - List items formatted with dashes on the same line
 * - Code blocks with opening brace on same line as delimiter
 * - JSON code blocks are formatted with proper indentation
 * 
 * @param {string} description - The description to fix
 * @returns {string} The fixed description with proper newlines and whitespace
 * 
 * @example
 * const broken = "Line 1\\n\\nLine 2\\nLine 3";
 * const fixed = fixDescription(broken);
 * // Result: "Line 1\n\nLine 2\nLine 3"
 */
function fixDescription(description: string): string {
    if (!description || typeof description !== 'string') {
        return description;
    }

    let fixed = description
        // Replace literal \n with actual newlines
        .replace(/\\n/g, '\n')
        // Replace literal \t with actual tabs
        .replace(/\\t/g, '\t')
        // Replace literal \r with actual carriage returns
        .replace(/\\r/g, '\r')
        // Remove unnecessary escape sequences but preserve meaningful ones
        // This handles cases where markdown code blocks might be escaped
        .replace(/\\`/g, '`');

    // Fix markdown code blocks to ensure proper formatting
    // Pattern matches complete code blocks: ```language content ```
    // Ensures: opening ends with newline, closing is on its own line
    // For JSON blocks, also formats the content prettily
    fixed = fixed.replace(/```(\w+)?([^`]*?)```/gs, (match, language, content) => {
        // Trim trailing whitespace from language specifier
        const lang = language ? language.trim() : '';
        // Ensure content starts with newline and ends with newline
        let cleanContent = content;

        // For JSON code blocks, format the JSON prettily
        if (lang.toLowerCase() === 'json') {
            cleanContent = formatJsonString(cleanContent.trim());
        }

        // Remove leading whitespace but preserve one newline
        if (!cleanContent.startsWith('\n')) {
            cleanContent = '\n' + cleanContent;
        }
        // Remove trailing whitespace but preserve one newline
        if (!cleanContent.endsWith('\n')) {
            cleanContent = cleanContent + '\n';
        }
        return '```' + lang + cleanContent + '```';
    });

    // Fix list items that are on the same line with dashes
    // Pattern: "text: - item1 - item2" -> "text:\n- item1\n- item2"
    fixed = fixed.replace(/([a-zA-Z0-9]:\s+)-\s+/g, '$1\n- ');

    // Fix subsequent list items on same line
    fixed = fixed.replace(/(\n-\s+[^\n]*)\s+-\s+/g, '$1\n- ');

    return fixed;
}

/**
 * Fix descriptions in an OpenAPI operation
 * 
 * @param {OpenAPIOperation} operation - The operation to fix
 * @returns {void} Modifies the operation in place
 */
function fixOperationDescriptions(operation: OpenAPIOperation | undefined): void {
    if (!operation) return;

    if (operation.description) {
        operation.description = fixDescription(operation.description);
    }
}

/**
 * Fix descriptions in an OpenAPI path item
 * 
 * @param {OpenAPIPathItem} pathItem - The path item to fix
 * @returns {void} Modifies the path item in place
 */
function fixPathItemDescriptions(pathItem: OpenAPIPathItem): void {
    const httpMethods = ['get', 'post', 'put', 'delete', 'patch', 'options', 'head', 'trace'];

    for (const method of httpMethods) {
        if (pathItem[method]) {
            fixOperationDescriptions(pathItem[method] as OpenAPIOperation);
        }
    }
}

/**
 * Fix all descriptions in an OpenAPI 3.0 document
 * 
 * Walks through the OpenAPI structure and fixes escaped characters
 * in all description fields. Modifies the document in place and returns it.
 * 
 * @param {OpenAPI3} doc - The OpenAPI 3.0 document to fix
 * @returns {OpenAPI3} The same document object (modified in place)
 * 
 * @example
 * const spec = await fetch('/openapi.json').then(r => r.json());
 * const fixed = fixOpenAPIDescriptions(spec);
 * // Now spec descriptions have proper newlines for markdown rendering
 */
export function fixOpenAPIDescriptions(doc: OpenAPI3): OpenAPI3 {
    if (!doc) return doc;

    // Fix info description
    if (doc.info?.description) {
        doc.info.description = fixDescription(doc.info.description);
    }

    // Fix all path descriptions
    if (doc.paths && typeof doc.paths === 'object') {
        for (const [pathKey, pathItem] of Object.entries(doc.paths)) {
            if (pathItem && typeof pathItem === 'object') {
                fixPathItemDescriptions(pathItem);
            }
        }
    }

    // Optionally fix schema descriptions in components
    if (doc.components?.schemas && typeof doc.components.schemas === 'object') {
        for (const [schemaKey, schema] of Object.entries(doc.components.schemas)) {
            if (schema && typeof schema === 'object' && 'description' in schema) {
                const schemaObj = schema as any;
                if (schemaObj.description) {
                    schemaObj.description = fixDescription(schemaObj.description);
                }
            }
        }
    }

    return doc;
}

/**
 * Alias for fixOpenAPIDescriptions for consistency with other functions
 * 
 * @param {OpenAPI3} doc - The OpenAPI 3.0 document to fix
 * @returns {OpenAPI3} The fixed document
 */
export const openapiFix = fixOpenAPIDescriptions;

export default fixOpenAPIDescriptions;

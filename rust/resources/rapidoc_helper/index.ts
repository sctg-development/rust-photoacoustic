/**
 * RapiDoc Helper - Code Snippet Generator for OpenAPI Documentation
 * 
 * This module provides a web-based interface for viewing OpenAPI/Swagger API documentation
 * using RapiDoc and automatically generates code snippets in multiple programming languages.
 * 
 * @module index
 * @version 1.0.0
 * @copyright 2024-2025 Ronan LE MEILLAT
 * @license AGPL-3.0-or-later
 */

import type { paths, components } from 'openapi3';
import { openapiAddSnippetsGeneric } from './src/openapi-snippets';
import { fixOpenAPIDescriptions } from './src/openapi-description-fixer';

/**
 * List of programming language targets and their corresponding HTTP client libraries
 * Each entry specifies a language (e.g., 'python', 'javascript') and the preferred 
 * client library for code generation (e.g., 'requests' for Python, 'axios' for JavaScript)
 * 
 * @type {Array<{language: string, client: string}>}
 * @const
 * 
 * Supported languages:
 * - c: C with libcurl HTTP client
 * - javascript: JavaScript/Node.js with Fetch API
 * - go: Go with native HTTP library
 * - rust: Rust with reqwest HTTP client
 * - python: Python 3 with native urllib
 * - shell: Bash shell with curl command
 */
const targets: Array<{ language: string; client: string }> = [
    { language: 'c', client: 'libcurl' },
    { language: 'javascript', client: 'fetch' },
    { language: 'go', client: 'native' },
    { language: 'rust', client: 'reqwest' },
    { language: 'python', client: 'python3' },
    { language: 'shell', client: 'curl' }
];

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
 * Initialize RapiDoc and generate code snippets for the OpenAPI specification
 * 
 * This try-catch block handles errors that may occur during RapiDoc web component loading.
 * Some errors from the json-schema-viewer library are benign and we suppress them.
 */
try {
    /**
     * Error handler for suppressing known harmless errors
     * 
     * The json-schema-viewer component may throw errors about undefined properties,
     * which don't affect functionality. This handler prevents these from cluttering
     * the browser console.
     * 
     * @param {ErrorEvent} event - The error event from the browser
     * @returns {void}
     */
    const errorHandler = (event: ErrorEvent) => {
        // Check if the error is from a known harmless source
        if (event.message.includes('Cannot set properties of undefined') ||
            event.message.includes('json-schema-viewer')) {
            // Prevent the error from propagating to the console
            event.preventDefault();
            console.warn('[index.ts] Caught json-schema-viewer error, continuing...');
        }
    };

    // Register the error handler for the capture phase
    // The 'true' parameter ensures errors are caught during the capture phase
    window.addEventListener('error', errorHandler, true);

    /**
     * Dynamically import the RapiDoc web component
     * 
     * RapiDoc is loaded dynamically to allow graceful degradation if it fails to load.
     * This is expected to work in modern browsers with support for dynamic imports.
     */
    import('@sctg/rapidoc').catch(err => {
        console.warn('[index.ts] Rapidoc import error (expected):', err.message);
    });

    /**
     * Remove the error handler after the RapiDoc initialization phase
     * 
     * We remove the error handler after 2 seconds to avoid suppressing errors
     * that may occur after RapiDoc has fully loaded.
     */
    setTimeout(() => {
        window.removeEventListener('error', errorHandler, true);
    }, 2000);
} catch (error) {
    console.warn('[index.ts] Failed to setup Rapidoc:', error);
}

/**
 * Wait for the DOM to be ready before initializing the application
 * 
 * The DOM may still be loading when this script runs. We use document.readyState
 * to check if the DOM is ready and use the appropriate initialization method.
 * 
 * In older browsers, readyState is 'loading' while the HTML document is still parsing.
 * We must wait for the 'DOMContentLoaded' event before accessing DOM elements.
 * 
 * In modern single-page applications, the DOM is usually already loaded, and we
 * can initialize immediately.
 */
if (document.readyState === 'loading') {
    /**
     * DOM is still loading, wait for the DOMContentLoaded event
     * 
     * This event fires when the HTML document has been completely parsed,
     * and all deferred scripts have executed, but before images and stylesheets
     * are fully loaded.
     */
    document.addEventListener('DOMContentLoaded', initializeApp);
} else {
    /**
     * DOM is already ready, initialize immediately
     * 
     * This typically happens in single-page applications where the script is
     * loaded after the DOM has already been parsed.
     */
    console.log('[index.ts] DOM already loaded, initializing immediately');
    initializeApp();
}

/**
 * Initialize the application by loading OpenAPI spec and generating code snippets
 * 
 * This is the main initialization function that:
 * 1. Waits for the RapiDoc web component to be registered
 * 2. Fetches the OpenAPI/Swagger specification from the server
 * 3. Converts API endpoints to HTTP Archive (HAR) format
 * 4. Generates code snippets in multiple programming languages
 * 5. Loads the API specification into RapiDoc for visualization
 * 
 * @async
 * @returns {Promise<void>}
 * @throws {Error} Errors are caught and logged but don't stop execution
 */
async function initializeApp() {
    try {
        console.log('[index.ts] Initializing app...');

        /**
         * Wait for the RapiDoc web component to be registered
         * 
         * The RapiDoc web component is loaded dynamically via dynamic import.
         * We need to give it time to be defined in the browser's custom elements registry.
         * 100 milliseconds is typically sufficient for this.
         */
        await new Promise(resolve => setTimeout(resolve, 100));

        /**
         * Get references to the RapiDoc element and the OpenAPI spec URL
         * 
         * The RapiDoc element is a web component with id="rapidoc" in the HTML.
         * The spec URL is passed to the window object by the server (typically via SPEC_URL)
         */
        const rapidocEl = document.getElementById('rapidoc') as any;

        const spec_url = (window as any).SPEC_URL;

        /**
         * Validate that the spec URL is defined
         * 
         * The spec URL must be provided by the HTML page or server configuration.
         * Without it, we cannot fetch the OpenAPI specification.
         */
        if (!spec_url) {
            console.error('[index.ts] SPEC_URL not set');
            return;
        }

        console.log('[index.ts] Fetching spec from:', spec_url);

        /**
         * Fetch the OpenAPI/Swagger specification from the server
         * 
         * The specification is typically served as JSON from a URL like:
         * - http://api.example.com/openapi.json
         * - http://api.example.com/swagger.json
         * 
         * @type {OpenAPI3}
         */
        const res = await fetch(spec_url);
        const data = await res.json() as OpenAPI3;

        /**
         * Validate that the specification contains API paths
         * 
         * A valid OpenAPI specification must have a 'paths' object
         * that describes the available API endpoints.
         */
        if (!data.paths) {
            console.error('[index.ts] Invalid spec data');
            // Still try to load the spec even if it's incomplete
            rapidocEl?.loadSpec(data);
            return;
        }

        /**
         * Set default host and base path if not specified in the spec
         * 
         * The 'host' field specifies the API server domain (e.g., 'api.example.com')
         * The 'basePath' field specifies the path prefix for all endpoints (e.g., '/v1')
         * 
         * If these are not in the OpenAPI spec, we use sensible defaults:
         * - host: the current page's domain (using window.location.host)
         * - basePath: '/' (root path)
         */
        if (data['host'] === undefined) {
            data['host'] = window.location.host;
        }
        if (data['basePath'] === undefined) {
            data['basePath'] = '/';
        }

        /**
         * Generate code snippets for all API endpoints
         * 
         * The openapiAddSnippetsGeneric function takes the OpenAPI spec and the list of targets,
         * generates code snippets for each endpoint in the specified programming languages,
         * and adds them to the specification under 'x-code-samples' extensions.
         */


        const dataWithSnippets = openapiAddSnippetsGeneric(fixOpenAPIDescriptions(data), targets);

        /**
         * Load the OpenAPI specification into RapiDoc
         * 
         * RapiDoc will render the specification as interactive API documentation
         * where users can:
         * - View endpoint details
         * - See request/response examples
         * - Try out API calls directly in the browser
         */
        console.log('[index.ts] Loading spec into Rapidoc...');
        rapidocEl?.loadSpec(dataWithSnippets);
        console.log('[index.ts] Spec loaded successfully');

        /**
         * Create download buttons that respect RapiDoc theme
         * 
         * Add compact download buttons to the RapiDoc footer slot
         * that follow RapiDoc styling and respect light/dark theme
         */
        const downloadButtonsHtml = `
            <span style="display: flex; gap: 0.5em; margin-left: auto;">
                <a href="${spec_url}" download="openapi.json" 
                   style="padding: 0.25em 0.75em; font-size: 0.85em; cursor: pointer; 
                           border: 1px solid var(--primary-color, #007bff);
                           color: var(--primary-color, #007bff);
                           background-color: transparent;
                           border-radius: 3px;
                           text-decoration: none;
                           transition: all 0.2s;"
                   onmouseover="this.style.backgroundColor='var(--primary-color, #007bff)'; this.style.color='white';"
                   onmouseout="this.style.backgroundColor='transparent'; this.style.color='var(--primary-color, #007bff)';">
                   📥 Original
                </a>
                <a id="download-with-snippets"
                   download="openapi-with-snippets.json" 
                   style="padding: 0.25em 0.75em; font-size: 0.85em; cursor: pointer;
                           border: 1px solid var(--primary-color, #28a745);
                           color: var(--primary-color, #28a745);
                           background-color: transparent;
                           border-radius: 3px;
                           text-decoration: none;
                           transition: all 0.2s;"
                   onmouseover="this.style.backgroundColor='var(--primary-color, #28a745)'; this.style.color='white';"
                   onmouseout="this.style.backgroundColor='transparent'; this.style.color='var(--primary-color, #28a745)';">
                   📥 With Snippets
                </a>
            </span>
        `;

        /**
         * Find the footer slot and inject the download buttons
         */
        const footerSlot = document.querySelector('[slot="footer"]');
        if (footerSlot) {
            const downloadSpan = document.createElement('span');
            downloadSpan.innerHTML = downloadButtonsHtml;
            // Insert download buttons at the end of footer content
            footerSlot.appendChild(downloadSpan);

            /**
             * Set up the blob URL for the "with snippets" download
             */
            const snippetsLink = footerSlot.querySelector('#download-with-snippets') as HTMLAnchorElement;
            if (snippetsLink) {
                const blob = new Blob([JSON.stringify(dataWithSnippets, null, 2)], { type: 'application/json' });
                const blobUrl = URL.createObjectURL(blob);
                snippetsLink.href = blobUrl;
            }
        } else {
            console.warn('[index.ts] Footer slot not found, download buttons not added');
        }
    } catch (error) {
        /**
         * Catch and log any unexpected errors
         * 
         * We log the error but don't rethrow it, allowing the application
         * to continue running even if initialization fails partially.
         */
        console.error('[index.ts] Error in initializeApp:', error);
    }
}


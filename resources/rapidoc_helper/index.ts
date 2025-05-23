/**
 * sctgdesk-server
 * Copyright (c) 2024 - Ronan LE MEILLAT
 * Licensed under Affero GPL v3
 */
import { OpenAPISnippets } from '@sctg/openapi-snippet';
import type { paths, components } from 'openapi3';

import '@sctg/rapidoc';
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

function getPrismLanguage(lang: string): string {
    if (lang === 'shell_curl') {
        return 'bash';
    }
    if (lang === 'javascript_fetch') {
        return 'javascript';
    }
    if (lang === 'c') {
        return 'c';
    }
    if (lang === 'php') {
        return 'php';
    }
    if (lang === 'go') {
        return 'go';
    }
    if (lang === 'rust') {
        return 'rust';
    }
    if (lang === 'python') {
        return 'python';
    }
    return lang;
}

window.addEventListener('DOMContentLoaded', (event) => {
    const rapidocEl = document.getElementById('rapidoc') as any;
    const spec_url = (window as any).SPEC_URL;
    const targets = ['c', 'javascript_fetch', 'go', 'php', 'rust', 'python', 'shell_curl'];
    fetch(spec_url)
        .then((res) => res.json() as Promise<OpenAPI3>)
        .then((data) => {
            if (data['host'] === undefined) {
                data['host'] = window.location.host;
            }
            if (data['basePath'] === undefined) {
                data['basePath'] = '/';
            }
            for (let path in data.paths) {
                for (let method in data.paths[path as keyof paths]) {
                    const snippets = OpenAPISnippets.getEndpointSnippets(data, path, method, targets);
                    const currentPath = data.paths[path as keyof paths];
                    const pathItem = currentPath[method as keyof typeof currentPath];
                    const code_samples = [];
                    for (let snippet of snippets.snippets) {
                        code_samples.push({
                            lang: getPrismLanguage(snippet.id),
                            label: snippet.title,
                            source: snippet.content
                        });
                        // console.log(JSON.stringify(code_samples[code_samples.length - 1], null, 2));
                    }
                    if (pathItem !== undefined && pathItem['x-code-samples' as keyof typeof pathItem] === undefined) {
                        (pathItem as any)['x-code-samples'] = code_samples;
                    }
                }
            }
            rapidocEl?.loadSpec(data);
        });
});
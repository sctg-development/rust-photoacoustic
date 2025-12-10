/**
 * sctgdesk-server
 * Copyright (c) 2024-2025 - Ronan LE MEILLAT
 * Licensed under Affero GPL v3
 */
import type { paths, components } from 'openapi3';

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

// Charger Rapidoc avec gestion d'erreur
try {
    // Capture les erreurs silencieuses de json-schema-viewer
    const errorHandler = (event: ErrorEvent) => {
        if (event.message.includes('Cannot set properties of undefined') || 
            event.message.includes('json-schema-viewer')) {
            event.preventDefault();
            console.warn('[index.ts] Caught json-schema-viewer error, continuing...');
        }
    };
    window.addEventListener('error', errorHandler, true);
    
    // Import Rapidoc
    import('@sctg/rapidoc').catch(err => {
        console.warn('[index.ts] Rapidoc import error (expected):', err.message);
    });
    
    // Nettoyer après 2 secondes
    setTimeout(() => {
        window.removeEventListener('error', errorHandler, true);
    }, 2000);
} catch (error) {
    console.warn('[index.ts] Failed to setup Rapidoc:', error);
}

// Attendre que le DOM soit prêt
if (document.readyState === 'loading') {
    // DOM pas encore prêt
    document.addEventListener('DOMContentLoaded', initializeApp);
} else {
    // DOM déjà prêt
    console.log('[index.ts] DOM already loaded, initializing immediately');
    initializeApp();
}

async function initializeApp() {
    try {
        console.log('[index.ts] Initializing app...');
        
        // Wait a bit for Rapidoc Web Component to be registered
        await new Promise(resolve => setTimeout(resolve, 100));
        
        const rapidocEl = document.getElementById('rapidoc') as any;
        const spec_url = (window as any).SPEC_URL;
        
        if (!spec_url) {
            console.error('[index.ts] SPEC_URL not set');
            return;
        }
        
        console.log('[index.ts] Fetching spec from:', spec_url);
        const res = await fetch(spec_url);
        const data = await res.json() as OpenAPI3;
        
        if (!data.paths) {
            console.error('[index.ts] Invalid spec data');
            rapidocEl?.loadSpec(data);
            return;
        }
        
        if (data['host'] === undefined) {
            data['host'] = window.location.host;
        }
        if (data['basePath'] === undefined) {
            data['basePath'] = '/';
        }
        
        console.log('[index.ts] Loading spec into Rapidoc...');
        rapidocEl?.loadSpec(data);
        console.log('[index.ts] Spec loaded successfully');
    } catch (error) {
        console.error('[index.ts] Error in initializeApp:', error);
    }
}

import { defineConfig } from 'vitepress'

const base = '/lap/'

export default defineConfig({
    base,
    title: "Lap",
    description: "Local-first, AI-powered photo manager",
    head: [
        ['link', { rel: 'icon', type: 'image/png', sizes: '512x512', href: `${base}icon.png` }],
        [
            'script',
            { async: '', src: 'https://www.googletagmanager.com/gtag/js?id=G-SVT0K4C2ET' }
        ],
        [
            'script',
            {},
            `window.dataLayer = window.dataLayer || [];
             function gtag(){dataLayer.push(arguments);}
             gtag('js', new Date());
             gtag('config', 'G-SVT0K4C2ET');`
        ]
    ],
    themeConfig: {
        logo: '/logo.png',
        nav: [
            { text: 'Home', link: '/' },
            { text: 'Guide', link: '/guide/introduction' },
            { text: 'Download', link: 'https://github.com/julyx10/lap/releases' }
        ],
        sidebar: [
            {
                text: 'Guide',
                items: [
                    { text: 'Introduction', link: '/guide/introduction' },
                    { text: 'Getting Started', link: '/guide/getting-started' },
                    { text: 'AI Plugin Interface', link: '/guide/ai-plugin-interface' },
                    { text: 'AI Plugin Roadmap', link: '/guide/ai-plugin-development-roadmap' },
                    { text: 'AI Runtime Status', link: '/guide/plugin-runtime-status-2026-06-20' },
                    { text: 'PicAiPic Progress', link: '/guide/picaipic-progress' }
                ]
            },
            {
                text: 'AI Plugins',
                items: [
                    { text: 'Current Status', link: '/ai-plugin-current-status' },
                    { text: 'Contract v1', link: '/ai-plugin-contract-v1' },
                    { text: 'Contract v1 Draft', link: '/ai-plugin-contract-v1-draft' },
                    { text: 'Author Checklist', link: '/ai-plugin-author-checklist' },
                    { text: 'E2E Regression 2026-06-30', link: '/ai-plugin-e2e-regression-2026-06-30' },
                    { text: 'UI Verification 2026-06-30', link: '/ai-plugin-ui-verification-2026-06-30' },
                    { text: 'Release Build 2026-06-30', link: '/release-build-2026-06-30' },
                    { text: 'Stop State Fix 2026-06-30', link: '/ai-plugin-stop-state-fix-2026-06-30' }
                ]
            },
            {
                text: 'Release Notes',
                items: [
                    { text: 'v0.2.4', link: '/guide/release-notes/v0.2.4' },
                    { text: 'v0.2.3', link: '/guide/release-notes/v0.2.3' },
                    { text: 'v0.2.2', link: '/guide/release-notes/v0.2.2' },
                    { text: 'v0.2.1', link: '/guide/release-notes/v0.2.1' },
                    { text: 'v0.2.0', link: '/guide/release-notes/v0.2.0' },
                    { text: 'v0.1.13', link: '/guide/release-notes/v0.1.13' },
                    { text: 'v0.1.12', link: '/guide/release-notes/v0.1.12' },
                    { text: 'v0.1.11', link: '/guide/release-notes/v0.1.11' },
                    { text: 'v0.1.10', link: '/guide/release-notes/v0.1.10' },
                    { text: 'v0.1.9', link: '/guide/release-notes/v0.1.9' },
                    { text: 'v0.1.8', link: '/guide/release-notes/v0.1.8' },
                    { text: 'v0.1.7', link: '/guide/release-notes/v0.1.7' },
                    { text: 'v0.1.6', link: '/guide/release-notes/v0.1.6' },
                    { text: 'v0.1.5', link: '/guide/release-notes/v0.1.5' },
                    { text: 'v0.1.4', link: '/guide/release-notes/v0.1.4' },
                    { text: 'v0.1.3', link: '/guide/release-notes/v0.1.3' },
                    { text: 'v0.1.2', link: '/guide/release-notes/v0.1.2' },
                    { text: 'v0.1.1', link: '/guide/release-notes/v0.1.1' },
                    { text: 'v0.1.0', link: '/guide/release-notes/v0.1.0' }
                ]
            }
        ],
        socialLinks: [
            { icon: 'github', link: 'https://github.com/julyx10/lap' }
        ],
        footer: {
            message: 'Released under the GPL-3.0 License.',
            copyright: 'Copyright © 2026 Lap Contributors'
        }
    }
})





import fs from 'node:fs';
import path from 'node:path';
import type { Plugin } from 'vite';

type Tool = { id: number; name: string; slug: string; category: string; description: string };
const escape = (text: string) => text.replace(/[&<>"']/g, (value) => ({
  '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
}[value]!));

export function seoPlugin(): Plugin {
  const root = import.meta.dirname;
  const tools: Tool[] = JSON.parse(fs.readFileSync(path.join(root, 'src/data/tools.json'), 'utf8'));
  const { publicUrl } = JSON.parse(fs.readFileSync(path.join(root, 'seo.config.json'), 'utf8'));
  return {
    name: 'dev-toolbox-static-seo',
    apply: 'build',
    enforce: 'post',
    generateBundle(_options, bundle) {
      const index = bundle['index.html'];
      if (!index || index.type !== 'asset') return;
      const template = String(index.source);
      const base = process.env.BASE_PATH || '/';
      const routeFor = (tool: Tool) => `${base}tool/${tool.id}/${tool.slug}`;
      const wrap = (content: string) => `<main style="max-width:1100px;margin:60px auto;padding:24px;font-family:system-ui;color:#173d33;line-height:1.6">${content}</main>`;
      const groups = [...new Set(tools.map((tool) => tool.category))];
      const catalog = wrap(
        '<h1>Free Developer Tools Directory</h1><p>Browse 710 planned developer and creator utilities. Every tool is coming soon; no tool functionality is available yet.</p>' +
        groups.map((category) => `<section><h2>${escape(category)}</h2><ul>${tools.filter((tool) => tool.category === category).map((tool) => `<li><a href="${routeFor(tool)}">${escape(tool.name)}</a> — Coming soon</li>`).join('')}</ul></section>`).join(''),
      );
      index.source = template.replace('<div id="root"></div>', `<div id="root">${catalog}</div>`);
      if (publicUrl) {
        index.source = String(index.source).replace('</head>', `<link rel="canonical" href="${escape(new URL(base, publicUrl).href)}"></head>`);
        this.emitFile({ type: 'asset', fileName: 'sitemap.xml', source: `<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>${escape(new URL(base, publicUrl).href)}</loc></url></urlset>` });
      }
      for (const tool of tools) {
        const title = `${tool.name} — Coming Soon | Dev Toolbox`;
        const description = `${tool.name} is a planned ${tool.category.toLowerCase()} utility in the free Dev Toolbox directory. This tool is coming soon.`;
        const content = wrap(`<nav><a href="${base}">Dev Toolbox directory</a></nav><article><p>${escape(tool.category)}</p><h1>${escape(tool.name)}</h1><p>${escape(description)}</p><h2>This tool is coming soon</h2><p>This listing is a preview only. The utility is not available yet.</p><a href="${base}">Browse all 710 planned tools</a></article>`);
        let html = template
          .replace(/<title>.*?<\/title>/, `<title>${escape(title)}</title>`)
          .replace(/(<meta (?:name="description"|property="og:description"|name="twitter:description") content=")[^"]*(")/g, `$1${escape(description)}$2`)
          .replace(/(<meta (?:property="og:title"|name="twitter:title") content=")[^"]*(")/g, `$1${escape(title)}$2`)
          .replace('content="index, follow"', 'content="noindex, follow"')
          .replace('<div id="root"></div>', `<div id="root">${content}</div>`);
        if (publicUrl) html = html.replace('</head>', `<link rel="canonical" href="${escape(new URL(routeFor(tool), publicUrl).href)}"></head>`);
        this.emitFile({ type: 'asset', fileName: `tool/${tool.id}/${tool.slug}/index.html`, source: html });
      }
    },
  };
}

import { ArrowLeft, Boxes, Clock3 } from 'lucide-react';
import { Link, useParams } from 'wouter';
import toolsData from '@/data/tools.json';
import { useSeo } from '@/lib/seo';

type Tool = (typeof toolsData)[number];

export default function ToolDetailPage() {
  const params = useParams<{ id: string; slug: string }>();
  const tool = toolsData.find((item) => item.id === Number(params.id) && item.slug === params.slug) as Tool | undefined;

  useSeo(
    tool ? `${tool.name} — Coming Soon | Dev Toolbox` : 'Tool not found — Dev Toolbox',
    tool
      ? `${tool.name} is a planned ${tool.category.toLowerCase()} utility in the free Dev Toolbox directory. Browse the catalog while this tool is being built.`
      : 'This utility is not listed in the Dev Toolbox catalog.',
    true,
  );

  return (
    <div className="page-shell">
      <header className="site-header">
        <Link href="/" className="brand" aria-label="Dev Toolbox home" data-testid="link-home">
          <span className="brand-mark" aria-hidden="true"><Boxes /></span><span>dev toolbox</span>
        </Link>
        <div className="header-note"><span className="header-dot" /> An independent directory</div>
      </header>
      <main className="main-wrap detail-wrap">
        <Link href="/" className="back-link" data-testid="link-back-catalog"><ArrowLeft size={16} /> Back to the catalog</Link>
        {tool ? (
          <article className="detail-card" data-testid={`detail-tool-${tool.id}`}>
            <div className="detail-overline"><span>{tool.category}</span><span aria-hidden="true">/</span><span>Tool no. {String(tool.id).padStart(3, '0')}</span></div>
            <h1>{tool.name}</h1>
            <p>{tool.description}</p>
            <section className="coming-panel" aria-labelledby="coming-title">
              <div className="coming-mark"><Clock3 size={18} /></div>
              <div>
                <h2 id="coming-title">This tool is coming soon</h2>
                <p>We’re building out the directory. This listing is a preview only; the utility is not available yet.</p>
              </div>
            </section>
            <div className="detail-meta">
              <span>Status <strong>Coming soon</strong></span>
              <span>Catalog ID <strong>DT-{String(tool.id).padStart(4, '0')}</strong></span>
            </div>
          </article>
        ) : (
          <section className="not-found" data-testid="status-tool-not-found">
            <div className="hero-kicker" style={{ justifyContent: 'center' }}>No listing found</div>
            <h1>That tool isn’t in the catalog.</h1>
            <p>Check the address or return to browse the planned utilities.</p>
            <Link href="/" className="back-link" data-testid="link-return-catalog"><ArrowLeft size={16} /> Browse the catalog</Link>
          </section>
        )}
        <footer className="site-footer">
          <span>Dev Toolbox <span aria-hidden="true">·</span> A directory in progress</span>
          <span>Browse freely. Every tool is coming soon.</span>
        </footer>
      </main>
    </div>
  );
}

import { useEffect, useMemo, useRef, useState } from 'react';
import { ArrowUpRight, Boxes, Command, Search, X } from 'lucide-react';
import { Link } from 'wouter';
import categoriesData from '@/data/categories.json';
import toolsData from '@/data/tools.json';
import { isWasmSearchReady, searchTools, type SearchTool } from '@/search/search';
import { useSeo } from '@/lib/seo';

import { ALL_TOOLS } from '../registry/tool-registry';

type Tool = SearchTool & { status?: string };
const tools = ALL_TOOLS as unknown as Tool[];
const allLabel = 'All tools';
const categories = [...new Map(categoriesData.map((item) => [item.name, item])).values()];
const pageSize = 24;

export default function CatalogPage() {
  const [query, setQuery] = useState('');
  const [category, setCategory] = useState('');
  const [resultIds, setResultIds] = useState<number[]>([]);
  const [visibleCount, setVisibleCount] = useState(pageSize);
  const [loading, setLoading] = useState(true);
  const [searchError, setSearchError] = useState(false);
  const [engineReady, setEngineReady] = useState(false);
  const [retry, setRetry] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useSeo('Free Developer Tools Directory — 710 Tools | Dev Toolbox', 'Find everyday developer and creator utilities in one free directory. Search 710 planned tools for code, PDFs, images, text, design, and more. Coming soon.');

  const categoryCounts = useMemo(() => {
    const counts = new Map<string, number>();
    tools.forEach((tool) => counts.set(tool.category, (counts.get(tool.category) || 0) + 1));
    return counts;
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === '/' && !event.metaKey && !event.ctrlKey && !event.altKey) {
        const target = event.target as HTMLElement;
        if (target.tagName !== 'INPUT' && target.tagName !== 'TEXTAREA' && !target.isContentEditable) {
          event.preventDefault();
          inputRef.current?.focus();
        }
      }
      if (event.key === 'Escape' && document.activeElement === inputRef.current) {
        setQuery('');
        inputRef.current?.blur();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  useEffect(() => {
    let active = true;
    const timer = window.setTimeout(() => {
      setLoading(true);
      setSearchError(false);
      searchTools(query, tools, category || undefined)
        .then((ids) => {
          if (!active) return;
          setResultIds(ids);
          setVisibleCount(pageSize);
          setEngineReady(isWasmSearchReady());
          setLoading(false);
        })
        .catch(() => {
          if (!active) return;
          setSearchError(true);
          setLoading(false);
        });
    }, query.trim() ? 180 : 0);
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [query, category, retry]);

  const resultTools = useMemo(() => {
    const byId = new Map(tools.map((tool) => [tool.id, tool]));
    return resultIds.map((id) => byId.get(id)).filter((tool): tool is Tool => Boolean(tool));
  }, [resultIds]);
  const visibleTools = resultTools.slice(0, visibleCount);

  return (
    <div className="page-shell">
      <header className="site-header">
        <Link href="/" className="brand" aria-label="Dev Toolbox home" data-testid="link-home">
          <span className="brand-mark" aria-hidden="true"><Boxes /></span>
          <span>dev toolbox</span>
        </Link>
        <div className="flex items-center gap-4">
          <Link
            href="/tracker"
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-semibold text-teal-800 bg-teal-50 hover:bg-teal-100 border border-teal-200/80 rounded-lg transition shadow-2xs"
          >
            <span>📊 Testing Tracker</span>
          </Link>
          <div className="header-note"><span className="header-dot" /> An independent directory</div>
        </div>
      </header>

      <main className="main-wrap">
        <section className="hero" aria-labelledby="catalog-title">
          <div className="hero-kicker">The practical web utility index</div>
          <h1 id="catalog-title">Developer tools.<br /><span>Without the noise.</span></h1>
          <p className="hero-copy">A growing directory of focused browser utilities for people who make things. Search once, find what you need, get back to work.</p>
          <div className="hero-meta">
            <span><strong>{tools.length.toLocaleString()}</strong> planned utilities</span>
            <span><strong>{categories.length}</strong> categories</span>
            <span><strong>Free</strong> to browse</span>
          </div>
          <div className="hero-art" aria-hidden="true"><Command size={34} strokeWidth={1.2} /></div>
        </section>

        <section className="catalog-layout" aria-label="Browse the tool catalog">
          <aside className="filters" aria-label="Filter by category">
            <div className="filter-heading">
              <span>Categories</span>
              {category && <button type="button" onClick={() => setCategory('')} data-testid="button-clear-category">Clear</button>}
            </div>
            <ul className="category-list">
              <li>
                <button className="category-button" type="button" aria-pressed={!category} onClick={() => setCategory('')} data-testid="filter-category-all">
                  <span>{allLabel}</span><span className="category-count">{tools.length}</span>
                </button>
              </li>
              {categories.map((item) => (
                <li key={item.name}>
                  <button className="category-button" type="button" aria-pressed={category === item.name} onClick={() => setCategory(category === item.name ? '' : item.name)} data-testid={`filter-category-${item.name.toLowerCase().replace(/[^a-z0-9]+/g, '-')}`}>
                    <span>{item.name}</span><span className="category-count">{categoryCounts.get(item.name) || 0}</span>
                  </button>
                </li>
              ))}
            </ul>
          </aside>

          <div className="catalog-main">
            <div className="search-box" role="search">
              <Search size={19} strokeWidth={1.8} aria-hidden="true" />
              <input
                ref={inputRef}
                id="tool-search"
                className="search-input"
                type="search"
                autoComplete="off"
                placeholder="Search tools, formats, or ideas..."
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                aria-label="Search the tool catalog"
                aria-describedby="search-help"
                data-testid="input-tool-search"
              />
              {query ? (
                <button type="button" className="clear-search" aria-label="Clear search" onClick={() => setQuery('')} data-testid="button-clear-search"><X size={16} /></button>
              ) : <kbd className="search-shortcut" aria-hidden="true">/</kbd>}
              <span id="search-help" className="sr-only">Press slash to focus search.</span>
            </div>

            <div className="result-toolbar" aria-live="polite">
              <div>
                <h2 className="result-title">{category || 'Browse all tools'}</h2>
                <p className="result-context">
                  {query ? `Matches for “${query}”` : 'Every listing is planned and marked coming soon.'}
                </p>
              </div>
              <span className="result-count" data-testid="text-result-count">
                {loading ? 'Searching…' : `${resultTools.length.toLocaleString()} ${resultTools.length === 1 ? 'result' : 'results'}`}
              </span>
            </div>

            {loading ? (
              <div className="tools-grid" aria-label="Loading catalog results" data-testid="status-search-loading">
                {Array.from({ length: 6 }, (_, index) => <div className="skeleton-card" key={index} />)}
              </div>
            ) : searchError ? (
              <div className="error-state" role="alert" data-testid="status-search-error">
                <div className="empty-icon"><Search size={19} /></div>
                <h3>Search could not finish</h3>
                <p>The catalog is still here. Try the search again in a moment.</p>
                <button className="text-action" type="button" onClick={() => setRetry((value) => value + 1)} data-testid="button-retry-search">Try again</button>
              </div>
            ) : resultTools.length === 0 ? (
              <div className="empty-state" data-testid="status-empty-results">
                <div className="empty-icon"><Search size={19} /></div>
                <h3>No matches in this corner</h3>
                <p>Try another phrase or choose a different category. The catalog is still growing.</p>
                {(query || category) && <button className="text-action" type="button" onClick={() => { setQuery(''); setCategory(''); }} data-testid="button-reset-filters">Clear search and filters</button>}
              </div>
            ) : (
              <>
                <div className="tools-grid" data-testid="list-tool-results">
                  {visibleTools.map((tool) => (
                    <Link href={`/tool/${tool.id}/${tool.slug}`} className="tool-card" key={tool.id} data-testid={`link-tool-${tool.id}`}>
                      <div className="tool-card-top">
                        <h3 className="tool-name">{tool.name}</h3>
                        <ArrowUpRight className="tool-arrow" size={17} aria-hidden="true" />
                      </div>
                      <p className="tool-desc">{tool.description}</p>
                      <div className="tool-card-bottom">
                        <span className="tool-category">{tool.category}</span>
                        <span
                          className={`status-pill ${tool.status === 'available' ? 'bg-teal-100 text-teal-800 border-teal-300 font-semibold' : ''}`}
                          data-testid={`status-tool-${tool.id}`}
                        >
                          {tool.status === 'available' ? 'Available' : 'Coming soon'}
                        </span>
                      </div>
                    </Link>
                  ))}
                </div>
                {visibleCount < resultTools.length && (
                  <button className="load-more" type="button" onClick={() => setVisibleCount((count) => count + pageSize)} data-testid="button-load-more">
                    Show more tools <span className="category-count">({(resultTools.length - visibleCount).toLocaleString()} remaining)</span>
                  </button>
                )}
                <div className="sr-only" aria-live="polite" data-testid="status-search-engine">
                  {engineReady ? 'Enhanced local search ready' : 'Local search available'}
                </div>
              </>
            )}
          </div>
        </section>
        <footer className="site-footer">
          <span>Dev Toolbox <span aria-hidden="true">·</span> A directory in progress</span>
          <span>Browse freely. Every tool is coming soon.</span>
        </footer>
      </main>
    </div>
  );
}

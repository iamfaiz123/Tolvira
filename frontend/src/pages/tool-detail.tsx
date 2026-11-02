import React, { Suspense } from "react";
import { ArrowLeft, Boxes, Clock3, Sparkles } from "lucide-react";
import { Link, useParams } from "wouter";
import { findToolById, findToolBySlug } from "../registry/tool-registry";
import { getToolComponent } from "../tools";
import { useSeo } from "../utils/seo";
import { useTracker } from "../services/tracker-store";

export default function ToolDetailPage() {
  const params = useParams<{ id: string; slug: string }>();
  const idNum = Number(params.id);
  const tool = findToolById(idNum) || findToolBySlug(params.slug);
  const { getStatus, updateStatus } = useTracker();
  const tracking = tool ? getStatus(tool.slug) : { status: "untested", notes: "" };

  const isAvailable = tool?.status === "available";
  const ToolComponent = isAvailable ? getToolComponent(tool?.ui?.component) : null;

  useSeo(
    tool
      ? `${tool.name} — ${isAvailable ? "Online Utility" : "Coming Soon"} | Dev Toolbox`
      : "Tool not found — Dev Toolbox",
    tool
      ? `${tool.name} is a ${tool.category.toLowerCase()} utility in the free Dev Toolbox directory.`
      : "This utility is not listed in the Dev Toolbox catalog.",
    !isAvailable,
  );

  return (
    <div className="page-shell">
      <header className="site-header">
        <Link href="/" className="brand" aria-label="Dev Toolbox home" data-testid="link-home">
          <span className="brand-mark" aria-hidden="true"><Boxes /></span><span>dev toolbox</span>
        </Link>
        <div className="flex items-center gap-4">
          <Link
            href="/tracker"
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-semibold text-teal-800 bg-teal-50 hover:bg-teal-100 border border-teal-200/80 rounded-lg transition shadow-2xs"
          >
            <span>📊 Testing Tracker</span>
          </Link>
          <div className="header-note">
            <span className="header-dot" /> Universal Online Utility Platform
          </div>
        </div>
      </header>
      <main className="main-wrap detail-wrap">
        <div className="flex items-center justify-between mb-4">
          <Link href="/" className="back-link" data-testid="link-back-catalog">
            <ArrowLeft size={16} /> Back to the catalog
          </Link>
          <Link href="/tracker" className="text-xs font-medium text-stone-500 hover:text-stone-800">
            View All in Tracker →
          </Link>
        </div>
        {tool ? (
          <article className="detail-card" data-testid={`detail-tool-${tool.id}`}>
            <div className="detail-overline">
              <span>{tool.category}</span>
              <span aria-hidden="true">/</span>
              <span>Tool no. {String(tool.id).padStart(3, "0")}</span>
            </div>
            <h1>{tool.name}</h1>
            <p>{tool.description}</p>

            {/* Live Testing Tracker Banner */}
            <div className="mt-5 mb-3 flex flex-wrap items-center justify-between gap-3 p-3 bg-stone-50 border border-stone-200 rounded-xl text-xs">
              <div className="flex items-center gap-2 flex-wrap">
                <span className="font-semibold text-stone-700">Testing Tracker:</span>
                <span className={`px-2 py-0.5 rounded font-mono font-medium text-[11px] ${
                  tracking.status === "passed"
                    ? "bg-emerald-100 text-emerald-800 border border-emerald-300"
                    : tracking.status === "fixed"
                    ? "bg-blue-100 text-blue-800 border border-blue-300"
                    : tracking.status === "issue"
                    ? "bg-amber-100 text-amber-800 border border-amber-300"
                    : "bg-stone-200 text-stone-700"
                }`}>
                  {tracking.status === "passed" && "✅ Tested & Passed"}
                  {tracking.status === "fixed" && "🔧 Fixed & Verified"}
                  {tracking.status === "issue" && "⚠️ Issue Found"}
                  {tracking.status === "untested" && "⏳ Untested"}
                  {tracking.status === "queued" && "💤 Queued"}
                </span>
                {tracking.notes && (
                  <span className="text-stone-500 italic max-w-sm truncate" title={tracking.notes}>
                    "{tracking.notes}"
                  </span>
                )}
              </div>
              <div className="flex items-center gap-1.5">
                <button
                  type="button"
                  onClick={() => updateStatus(tool.slug, "passed")}
                  className={`px-2.5 py-1 rounded text-xs font-semibold transition ${
                    tracking.status === "passed"
                      ? "bg-emerald-700 text-white"
                      : "bg-emerald-50 text-emerald-800 hover:bg-emerald-100 border border-emerald-200"
                  }`}
                  title="Mark this tool as tested & working"
                >
                  ✓ Mark Done
                </button>
                <button
                  type="button"
                  onClick={() => {
                    const note = prompt("Details on what was fixed:", tracking.notes || "");
                    if (note !== null) updateStatus(tool.slug, "fixed", note);
                  }}
                  className={`px-2.5 py-1 rounded text-xs font-semibold transition ${
                    tracking.status === "fixed"
                      ? "bg-blue-700 text-white"
                      : "bg-blue-50 text-blue-800 hover:bg-blue-100 border border-blue-200"
                  }`}
                  title="Mark this tool as fixed and verified"
                >
                  🔧 Mark Fixed
                </button>
                <button
                  type="button"
                  onClick={() => {
                    const note = prompt("Describe the issue / bug found:", tracking.notes || "");
                    if (note !== null) updateStatus(tool.slug, "issue", note);
                  }}
                  className={`px-2.5 py-1 rounded text-xs font-semibold transition ${
                    tracking.status === "issue"
                      ? "bg-amber-700 text-white"
                      : "bg-amber-50 text-amber-800 hover:bg-amber-100 border border-amber-200"
                  }`}
                  title="Flag that this tool has an issue"
                >
                  ⚠️ Report Issue
                </button>
              </div>
            </div>

            {/* Interactive Tool Implementation */}
            {isAvailable && ToolComponent ? (
              <div className="my-6">
                <div className="mb-3 flex items-center gap-1.5 text-xs font-semibold text-teal-700 dark:text-teal-400">
                  <Sparkles size={14} />
                  <span>Interactive Client Utility (WASM Powered)</span>
                </div>
                <Suspense
                  fallback={
                    <div className="rounded-xl border border-stone-200 bg-stone-50 p-8 text-center text-sm text-stone-500">
                      Loading processing engine...
                    </div>
                  }
                >
                  <ToolComponent tool={tool} />
                </Suspense>
              </div>
            ) : (
              <section className="coming-panel" aria-labelledby="coming-title">
                <div className="coming-mark"><Clock3 size={18} /></div>
                <div>
                  <h2 id="coming-title">This tool is coming soon</h2>
                  <p>We’re building out the universal platform. This utility engine is queued for development.</p>
                </div>
              </section>
            )}

            <div className="detail-meta">
              <span>
                Status{" "}
                <strong className={isAvailable ? "text-teal-700" : ""}>
                  {isAvailable ? "Available" : "Coming soon"}
                </strong>
              </span>
              <span>
                Catalog ID <strong>DT-{String(tool.id).padStart(4, "0")}</strong>
              </span>
              {tool.engine && (
                <span>
                  Engine <strong>{tool.engine.id} (v{tool.engine.version})</strong>
                </span>
              )}
            </div>
          </article>
        ) : (
          <section className="not-found" data-testid="status-tool-not-found">
            <div className="hero-kicker" style={{ justifyContent: "center" }}>No listing found</div>
            <h1>That tool isn’t in the catalog.</h1>
            <p>Check the address or return to browse the utilities.</p>
            <Link href="/" className="back-link" data-testid="link-return-catalog">
              <ArrowLeft size={16} /> Browse the catalog
            </Link>
          </section>
        )}
        <footer className="site-footer">
          <span>Dev Toolbox <span aria-hidden="true">·</span> Universal Utility Platform</span>
          <span>100% client-side privacy. Fast WASM execution.</span>
        </footer>
      </main>
    </div>
  );
}

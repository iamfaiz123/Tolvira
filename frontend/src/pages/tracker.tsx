import React, { useMemo, useState } from "react";
import {
  AlertTriangle,
  ArrowLeft,
  Boxes,
  CheckCircle2,
  Clock,
  Download,
  ExternalLink,
  Search,
  Sparkles,
  Wrench,
  X,
} from "lucide-react";
import { Link } from "wouter";
import { ALL_TOOLS } from "../registry/tool-registry";
import { type TestStatus, useTracker } from "../services/tracker-store";
import { useSeo } from "../utils/seo";

export default function TrackerPage() {
  const { tracker, updateStatus, exportJson } = useTracker();
  const [search, setSearch] = useState("");
  const [selectedCategory, setSelectedCategory] = useState("all");
  const [selectedStatus, setSelectedStatus] = useState<"all" | TestStatus>("all");
  const [editingNotes, setEditingNotes] = useState<Record<string, string>>({});

  useSeo(
    "Tool Testing & Fix Tracker — Dev Toolbox",
    "Track testing, verification, and bug fixes across all 1,000+ utilities.",
    true
  );

  const categories = useMemo(() => {
    const set = new Set<string>();
    ALL_TOOLS.forEach((t) => set.add(t.category));
    return Array.from(set).sort();
  }, []);

  const stats = useMemo(() => {
    let passed = 0;
    let fixed = 0;
    let issue = 0;
    let untested = 0;
    let available = 0;

    ALL_TOOLS.forEach((t) => {
      const isLive = t.status === "available";
      if (isLive) available++;
      const st = tracker[t.slug]?.status || (isLive ? "untested" : "queued");
      if (st === "passed") passed++;
      else if (st === "fixed") fixed++;
      else if (st === "issue") issue++;
      else if (st === "untested" && isLive) untested++;
    });

    const verified = passed + fixed;
    const progressPct = available > 0 ? (verified / available) * 100 : 0;

    return { total: ALL_TOOLS.length, available, passed, fixed, issue, untested, verified, progressPct };
  }, [tracker]);

  const filteredTools = useMemo(() => {
    return ALL_TOOLS.filter((t) => {
      const st = tracker[t.slug]?.status || (t.status === "available" ? "untested" : "queued");

      // Status filter
      if (selectedStatus !== "all" && st !== selectedStatus) {
        return false;
      }

      // Category filter
      if (selectedCategory !== "all" && t.category !== selectedCategory) {
        return false;
      }

      // Search filter
      if (search.trim()) {
        const q = search.toLowerCase();
        const note = (tracker[t.slug]?.notes || "").toLowerCase();
        const matchName = t.name.toLowerCase().includes(q);
        const matchSlug = t.slug.toLowerCase().includes(q);
        const matchId = String(t.id).includes(q);
        const matchEng = (t.engine?.id || "").toLowerCase().includes(q);
        const matchNote = note.includes(q);
        if (!matchName && !matchSlug && !matchId && !matchEng && !matchNote) {
          return false;
        }
      }

      return true;
    });
  }, [search, selectedCategory, selectedStatus, tracker]);

  const handleNoteChange = (slug: string, val: string) => {
    setEditingNotes((prev) => ({ ...prev, [slug]: val }));
  };

  const handleNoteBlur = (slug: string, currentStatus: TestStatus) => {
    if (editingNotes[slug] !== undefined) {
      updateStatus(slug, currentStatus, editingNotes[slug]);
    }
  };

  const handleExport = () => {
    const dataStr = "data:text/json;charset=utf-8," + encodeURIComponent(exportJson());
    const dlAnchor = document.createElement("a");
    dlAnchor.setAttribute("href", dataStr);
    dlAnchor.setAttribute("download", `tracker-${new Date().toISOString().slice(0, 10)}.json`);
    dlAnchor.click();
  };

  return (
    <div className="page-shell bg-[#fbf9f4] min-h-screen">
      <header className="site-header sticky top-0 z-20 backdrop-blur-md">
        <Link href="/" className="brand" aria-label="Dev Toolbox home">
          <span className="brand-mark" aria-hidden="true">
            <Boxes />
          </span>
          <span>dev toolbox</span>
        </Link>
        <div className="flex items-center gap-3">
          <span className="text-xs font-mono text-stone-600 bg-stone-100 px-2.5 py-1 rounded-md border border-stone-200">
            {stats.verified} / {stats.available} Verified ({stats.progressPct.toFixed(1)}%)
          </span>
          <button
            onClick={handleExport}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-stone-700 bg-white border border-stone-300 rounded-lg hover:bg-stone-50 transition shadow-sm"
            title="Download tracker state as JSON"
          >
            <Download size={14} />
            <span>Export JSON</span>
          </button>
        </div>
      </header>

      <main className="main-wrap detail-wrap max-w-none px-4 md:px-10 py-8">
        <div className="flex flex-wrap items-center justify-between gap-4 mb-6">
          <div>
            <Link href="/" className="back-link mb-2 text-stone-600 hover:text-stone-900 inline-flex items-center gap-1.5 text-xs font-semibold">
              <ArrowLeft size={14} /> Back to Catalog
            </Link>
            <h1 className="text-3xl font-bold tracking-tight text-stone-900">
              Tool Testing & Fix Tracker
            </h1>
            <p className="text-sm text-stone-600 mt-1">
              Verify utilities in the browser, record bug fixes, and track operational coverage across all {ALL_TOOLS.length} tools.
            </p>
          </div>
        </div>

        {/* Top Metric Cards */}
        <div className="grid grid-cols-2 md:grid-cols-5 gap-3.5 mb-8">
          <div className="bg-white border border-stone-200 rounded-xl p-4 shadow-sm">
            <div className="text-xs font-medium text-stone-500 uppercase tracking-wider">Overall Progress</div>
            <div className="text-2xl font-extrabold text-stone-900 mt-1">{stats.progressPct.toFixed(1)}%</div>
            <div className="w-full bg-stone-100 h-2 rounded-full mt-2.5 overflow-hidden">
              <div
                className="bg-emerald-600 h-full rounded-full transition-all duration-300"
                style={{ width: `${stats.progressPct}%` }}
              />
            </div>
            <div className="text-[11px] text-stone-500 mt-1.5">{stats.verified} of {stats.available} live tools</div>
          </div>

          <div
            onClick={() => setSelectedStatus(selectedStatus === "passed" ? "all" : "passed")}
            className={`bg-white border rounded-xl p-4 shadow-sm cursor-pointer transition ${
              selectedStatus === "passed" ? "ring-2 ring-emerald-500 border-emerald-500 bg-emerald-50/20" : "border-stone-200 hover:border-emerald-300"
            }`}
          >
            <div className="flex items-center justify-between text-xs font-medium text-emerald-700">
              <span>Passed & Working</span>
              <CheckCircle2 size={16} />
            </div>
            <div className="text-2xl font-extrabold text-emerald-800 mt-1">{stats.passed}</div>
            <div className="text-[11px] text-emerald-600 mt-1.5">Tested & operational</div>
          </div>

          <div
            onClick={() => setSelectedStatus(selectedStatus === "fixed" ? "all" : "fixed")}
            className={`bg-white border rounded-xl p-4 shadow-sm cursor-pointer transition ${
              selectedStatus === "fixed" ? "ring-2 ring-blue-500 border-blue-500 bg-blue-50/20" : "border-stone-200 hover:border-blue-300"
            }`}
          >
            <div className="flex items-center justify-between text-xs font-medium text-blue-700">
              <span>Fixed & Verified</span>
              <Wrench size={16} />
            </div>
            <div className="text-2xl font-extrabold text-blue-800 mt-1">{stats.fixed}</div>
            <div className="text-[11px] text-blue-600 mt-1.5">Bug resolved & re-tested</div>
          </div>

          <div
            onClick={() => setSelectedStatus(selectedStatus === "issue" ? "all" : "issue")}
            className={`bg-white border rounded-xl p-4 shadow-sm cursor-pointer transition ${
              selectedStatus === "issue" ? "ring-2 ring-amber-500 border-amber-500 bg-amber-50/20" : "border-stone-200 hover:border-amber-300"
            }`}
          >
            <div className="flex items-center justify-between text-xs font-medium text-amber-700">
              <span>Needs Fix</span>
              <AlertTriangle size={16} />
            </div>
            <div className="text-2xl font-extrabold text-amber-800 mt-1">{stats.issue}</div>
            <div className="text-[11px] text-amber-600 mt-1.5">Flagged for debugging</div>
          </div>

          <div
            onClick={() => setSelectedStatus(selectedStatus === "untested" ? "all" : "untested")}
            className={`bg-white border rounded-xl p-4 shadow-sm cursor-pointer transition ${
              selectedStatus === "untested" ? "ring-2 ring-stone-500 border-stone-500 bg-stone-50" : "border-stone-200 hover:border-stone-300"
            }`}
          >
            <div className="flex items-center justify-between text-xs font-medium text-stone-600">
              <span>Untested Live</span>
              <Clock size={16} />
            </div>
            <div className="text-2xl font-extrabold text-stone-800 mt-1">{stats.untested}</div>
            <div className="text-[11px] text-stone-500 mt-1.5">Awaiting browser test</div>
          </div>
        </div>

        {/* Filters and Search Bar */}
        <div className="bg-white border border-stone-200 rounded-xl p-4 shadow-sm mb-6 flex flex-wrap items-center justify-between gap-4">
          <div className="flex flex-wrap items-center gap-2">
            <button
              onClick={() => setSelectedStatus("all")}
              className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition ${
                selectedStatus === "all"
                  ? "bg-stone-900 text-white"
                  : "bg-stone-100 text-stone-700 hover:bg-stone-200"
              }`}
            >
              All Tools ({ALL_TOOLS.length})
            </button>
            <button
              onClick={() => setSelectedStatus("untested")}
              className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition ${
                selectedStatus === "untested"
                  ? "bg-stone-800 text-white"
                  : "bg-stone-100 text-stone-700 hover:bg-stone-200"
              }`}
            >
              Untested ({stats.untested})
            </button>
            <button
              onClick={() => setSelectedStatus("issue")}
              className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition ${
                selectedStatus === "issue"
                  ? "bg-amber-600 text-white"
                  : "bg-amber-50 text-amber-800 hover:bg-amber-100 border border-amber-200"
              }`}
            >
              Needs Fix ({stats.issue})
            </button>
            <button
              onClick={() => setSelectedStatus("fixed")}
              className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition ${
                selectedStatus === "fixed"
                  ? "bg-blue-600 text-white"
                  : "bg-blue-50 text-blue-800 hover:bg-blue-100 border border-blue-200"
              }`}
            >
              Fixed ({stats.fixed})
            </button>
            <button
              onClick={() => setSelectedStatus("passed")}
              className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition ${
                selectedStatus === "passed"
                  ? "bg-emerald-600 text-white"
                  : "bg-emerald-50 text-emerald-800 hover:bg-emerald-100 border border-emerald-200"
              }`}
            >
              Passed ({stats.passed})
            </button>
          </div>

          <div className="flex items-center gap-3 flex-1 min-w-[280px] max-w-xl">
            {/* Category Select */}
            <select
              value={selectedCategory}
              onChange={(e) => setSelectedCategory(e.target.value)}
              className="text-xs bg-stone-50 border border-stone-300 rounded-lg px-2.5 py-2 font-medium text-stone-800 focus:outline-none focus:ring-2 focus:ring-teal-600"
            >
              <option value="all">All Categories ({categories.length})</option>
              {categories.map((c) => (
                <option key={c} value={c}>
                  {c}
                </option>
              ))}
            </select>

            {/* Search Input */}
            <div className="relative flex-1">
              <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-stone-400" />
              <input
                type="text"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Search tools by name, ID, slug, engine, or notes..."
                className="w-full text-xs pl-8 pr-8 py-2 bg-stone-50 border border-stone-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-teal-600 text-stone-900 placeholder-stone-400"
              />
              {search && (
                <button
                  onClick={() => setSearch("")}
                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-stone-400 hover:text-stone-600"
                >
                  <X size={14} />
                </button>
              )}
            </div>
          </div>
        </div>

        {/* Tools Listing Table */}
        <div className="bg-white border border-stone-200 rounded-xl shadow-sm overflow-hidden">
          <div className="px-5 py-3 border-b border-stone-200 bg-stone-50 flex items-center justify-between text-xs text-stone-600 font-medium">
            <span>Showing {filteredTools.length} of {ALL_TOOLS.length} utilities</span>
            <span>Click any tool name to test it directly in browser</span>
          </div>

          <div className="overflow-x-auto">
            <table className="w-full text-left border-collapse text-xs">
              <thead>
                <tr className="border-b border-stone-200 text-stone-500 font-mono text-[11px] uppercase tracking-wider bg-stone-50/50">
                  <th className="py-3 px-4 w-12 text-center">ID</th>
                  <th className="py-3 px-4 min-w-[200px]">Tool Name</th>
                  <th className="py-3 px-4 min-w-[140px]">Category</th>
                  <th className="py-3 px-4 min-w-[140px]">Engine</th>
                  <th className="py-3 px-4 min-w-[220px]">Testing Status</th>
                  <th className="py-3 px-4 min-w-[260px]">Notes & Verification</th>
                  <th className="py-3 px-4 w-24 text-right">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-stone-100">
                {filteredTools.length === 0 ? (
                  <tr>
                    <td colSpan={7} className="py-12 text-center text-stone-500">
                      No tools match the selected filters.
                    </td>
                  </tr>
                ) : (
                  filteredTools.map((tool) => {
                    const info = tracker[tool.slug] || {
                      status: tool.status === "available" ? "untested" : "queued",
                      notes: "",
                    };
                    const isLive = tool.status === "available";
                    const currentNote =
                      editingNotes[tool.slug] !== undefined
                        ? editingNotes[tool.slug]
                        : info.notes || "";

                    return (
                      <tr key={tool.id} className="hover:bg-stone-50/70 transition-colors">
                        <td className="py-3 px-4 font-mono text-stone-400 text-center">
                          {tool.id}
                        </td>
                        <td className="py-3 px-4">
                          <Link
                            href={`/tool/${tool.id}/${tool.slug}`}
                            className="font-bold text-stone-900 hover:text-teal-700 inline-flex items-center gap-1.5 group"
                          >
                            <span>{tool.name}</span>
                            <ExternalLink size={12} className="opacity-0 group-hover:opacity-100 text-stone-400 transition" />
                          </Link>
                          <div className="font-mono text-[10px] text-stone-400 mt-0.5">
                            {tool.slug}
                          </div>
                        </td>
                        <td className="py-3 px-4 text-stone-600 font-medium">
                          {tool.category}
                        </td>
                        <td className="py-3 px-4">
                          {tool.engine ? (
                            <span className="inline-flex items-center gap-1 font-mono text-[10px] bg-stone-100 text-stone-700 px-2 py-0.5 rounded border border-stone-200">
                              <Sparkles size={10} className="text-teal-600" />
                              {tool.engine.id}
                            </span>
                          ) : (
                            <span className="font-mono text-[10px] text-stone-400">
                              {isLive ? "Client JS" : "Pending"}
                            </span>
                          )}
                        </td>
                        <td className="py-3 px-4">
                          <div className="flex items-center gap-1.5 flex-wrap">
                            <button
                              onClick={() => updateStatus(tool.slug, "passed")}
                              className={`px-2 py-1 rounded text-[11px] font-semibold transition ${
                                info.status === "passed"
                                  ? "bg-emerald-600 text-white shadow-xs"
                                  : "bg-stone-100 text-stone-600 hover:bg-emerald-50 hover:text-emerald-700"
                              }`}
                              title="Mark as Tested & Working"
                            >
                              ✓ Pass
                            </button>
                            <button
                              onClick={() => updateStatus(tool.slug, "fixed")}
                              className={`px-2 py-1 rounded text-[11px] font-semibold transition ${
                                info.status === "fixed"
                                  ? "bg-blue-600 text-white shadow-xs"
                                  : "bg-stone-100 text-stone-600 hover:bg-blue-50 hover:text-blue-700"
                              }`}
                              title="Mark as Fixed & Verified"
                            >
                              🔧 Fixed
                            </button>
                            <button
                              onClick={() => updateStatus(tool.slug, "issue")}
                              className={`px-2 py-1 rounded text-[11px] font-semibold transition ${
                                info.status === "issue"
                                  ? "bg-amber-600 text-white shadow-xs"
                                  : "bg-stone-100 text-stone-600 hover:bg-amber-50 hover:text-amber-700"
                              }`}
                              title="Flag as Needs Fix / Bug Found"
                            >
                              ⚠️ Issue
                            </button>
                            {info.status !== "untested" && isLive && (
                              <button
                                onClick={() => updateStatus(tool.slug, "untested")}
                                className="px-1.5 py-1 text-[10px] text-stone-400 hover:text-stone-700 transition"
                                title="Reset to Untested"
                              >
                                Reset
                              </button>
                            )}
                          </div>
                        </td>
                        <td className="py-3 px-4">
                          <input
                            type="text"
                            value={currentNote}
                            onChange={(e) => handleNoteChange(tool.slug, e.target.value)}
                            onBlur={() => handleNoteBlur(tool.slug, info.status)}
                            onKeyDown={(e) => {
                              if (e.key === "Enter") {
                                (e.target as HTMLInputElement).blur();
                              }
                            }}
                            placeholder="Add testing note or fix details..."
                            className="w-full bg-transparent hover:bg-stone-100/60 focus:bg-white border border-transparent hover:border-stone-200 focus:border-stone-400 rounded px-2 py-1 text-xs text-stone-800 placeholder-stone-300 focus:outline-none transition"
                          />
                        </td>
                        <td className="py-3 px-4 text-right">
                          <Link
                            href={`/tool/${tool.id}/${tool.slug}`}
                            className="inline-flex items-center gap-1 text-[11px] font-medium text-teal-700 bg-teal-50 hover:bg-teal-100 border border-teal-200 px-2.5 py-1 rounded transition"
                          >
                            <span>Open</span>
                            <ExternalLink size={10} />
                          </Link>
                        </td>
                      </tr>
                    );
                  })
                )}
              </tbody>
            </table>
          </div>
        </div>
      </main>
    </div>
  );
}


import React from "react";
import { Link } from "wouter";

export const Footer: React.FC = () => {
  return (
    <footer className="site-footer border-t border-stone-200/60 py-8 text-center text-xs text-stone-500 dark:border-stone-800">
      <div className="container mx-auto px-4">
        <p className="mb-2">
          Universal Online Utility Platform — client-side WASM & Web Worker architecture.
        </p>
        <p className="text-stone-400">
          Fast, private, and secure. Zero file uploads for client utilities.
        </p>
      </div>
    </footer>
  );
};

export default Footer;


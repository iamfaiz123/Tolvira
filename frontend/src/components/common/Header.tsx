import React from "react";
import { Link } from "wouter";
import { Boxes } from "lucide-react";

export const Header: React.FC = () => {
  return (
    <header className="site-header">
      <Link href="/" className="brand" aria-label="Dev Toolbox Home">
        <span className="brand-mark">
          <Boxes className="h-5 w-5" />
        </span>
        <span className="brand-text">
          <span className="brand-title">Dev Toolbox</span>
          <span className="brand-tag">710 tools indexed</span>
        </span>
      </Link>
    </header>
  );
};

export default Header;


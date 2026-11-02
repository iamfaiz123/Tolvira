import React, { type ReactNode } from "react";
import Header from "../components/common/Header";
import Footer from "../components/common/Footer";

interface Props {
  children: ReactNode;
}

export const MainLayout: React.FC<Props> = ({ children }) => {
  return (
    <div className="page-shell min-h-screen flex flex-col">
      <Header />
      <main className="flex-1">{children}</main>
      <Footer />
    </div>
  );
};

export default MainLayout;


import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { SettingsView } from "./components/SettingsView";
import { applyTheme } from "./theme";
import "./styles/globals.css";

// Tránh nháy sáng trên máy theme tối trước khi settings được nạp
applyTheme("system");

// Cửa sổ "settings" là cửa sổ Windows riêng -> render thẳng trang cài đặt.
const label = getCurrentWindow().label;

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {label === "settings" ? (
      <SettingsView onClose={() => void getCurrentWindow().hide()} />
    ) : (
      <App />
    )}
  </React.StrictMode>
);

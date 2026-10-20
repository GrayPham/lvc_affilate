import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import LicenseGate from "./components/LicenseGate";
import "./index.css";
import { startEngineBridge } from "./store/engine_bridge";

// Dang ky kenh `engine://event` MOT LAN cho ca vong doi app, TRUOC khi render — khong phu thuoc
// tab nao dang mo (xem doc `store/engine_bridge.ts`, review fix-round 1 C2).
startEngineBridge();

const rootEl = document.getElementById("root");
if (!rootEl) {
  throw new Error("khong tim thay #root trong index.html");
}

createRoot(rootEl).render(
  <StrictMode>
    <LicenseGate>
      <App />
    </LicenseGate>
  </StrictMode>,
);

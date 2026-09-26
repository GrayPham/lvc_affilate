// Dây nối "dòng chảy" (P-lumina) — nét ĐỨT + một ĐỐM SÁNG chạy dọc theo đường từ node nguồn -> đích
// để thể hiện HƯỚNG đi của dữ liệu. Dùng SVG <animateMotion> chạy theo chính path bezier của cạnh.
import { BaseEdge, getBezierPath, type EdgeProps } from "@xyflow/react";

export default function FlowEdge({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  markerEnd,
  style,
}: EdgeProps) {
  const [path] = getBezierPath({
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
  });

  return (
    <>
      <BaseEdge
        id={id}
        path={path}
        markerEnd={markerEnd}
        style={{ stroke: "#6b6b74", strokeWidth: 1.5, strokeDasharray: "6 6", ...style }}
      />
      {/* Đốm sáng chạy theo hướng nguồn -> đích (glow qua drop-shadow). */}
      <circle r="3.4" fill="#7dd3fc" style={{ filter: "drop-shadow(0 0 4px #38bdf8)" }}>
        <animateMotion dur="2.4s" repeatCount="indefinite" path={path} rotate="auto" />
      </circle>
    </>
  );
}

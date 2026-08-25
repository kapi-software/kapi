// Dock 弧线数学：左半弧槽位几何与滚轮循环索引（纯函数，可单元测试）
// Dock arc math: left-semicircle slot geometry and scroll-cycling indexes (pure, testable)
// 规格见 docs/DOCK.md §2：圆心 (320,280)、半径 260、两端各留 10% 边距

// Dock 容器固定尺寸（docs/DOCK.md §1：320 × 560，永不 resize）
// Fixed dock container size (docs/DOCK.md §1: 320 x 560, never resized)
export const DOCK_WIDTH = 320
export const DOCK_HEIGHT = 560

// 弧线几何常量
// Arc geometry constants
const CENTER_X = DOCK_WIDTH // 圆心在容器右缘
const CENTER_Y = DOCK_HEIGHT / 2 // 垂直中心
const RADIUS = 260
// 角度范围：左半弧，上下各留 10% 边距
// Angle range: left half arc with 10% margin at both ends
const THETA_START = Math.PI / 2 + 0.1 * Math.PI
const THETA_END = (3 * Math.PI) / 2 - 0.1 * Math.PI

// 弧线上的一个插件槽位
// One plugin slot on the arc
export interface DockPosition {
  // 槽位中心坐标（容器局部坐标，64px 圆点的圆心）
  // Slot center (container-local, center of the 64px dot)
  x: number
  y: number
  // 可见槽位序号（0 = 弧线最下端）
  // Visible slot index (0 = bottom end of the arc)
  slotIndex: number
  // 该槽位当前承载的插件索引（滚轮循环后）
  // Plugin index carried by this slot (after scroll cycling)
  actualIndex: number
  // 是否居中选中位
  // Whether this is the centered selected slot
  isActive: boolean
}

// 弧线基础几何：第 index / (total-1) 个槽位的圆心坐标
// Base arc geometry: center point of slot index / (total-1)
// getPositionOnArc(0, 9) => { x: ~239.7, y: ~527.3 }（最下端槽位）
// getPositionOnArc(0, 9) => { x: ~239.7, y: ~527.3 } (bottom slot)
export function getPositionOnArc(index: number, total: number): { x: number; y: number } {
  // 单槽位守卫：total<=1 时固定在最左点（弧线视觉中心），避免除零
  // Single-slot guard: pin to the leftmost point (visual arc center) to avoid division by zero
  const t = total <= 1 ? 0.5 : index / (total - 1)
  const theta = THETA_START + t * (THETA_END - THETA_START)
  return {
    x: CENTER_X + RADIUS * Math.cos(theta),
    y: CENTER_Y + RADIUS * Math.sin(theta),
  }
}

// Dock 贴靠边：right = 弧线凸向左（贴右缘），left = 镜像凸向右（贴左缘）
// Dock attach side: right = arc bulges left; left = mirrored arc bulges right
export type DockSide = 'right' | 'left'

// 滚轮循环：任意偏移归一到 [0, total)
// Scroll cycling: normalize any offset into [0, total)
// cycleOffset(100, 9) => 1; cycleOffset(-1, 9) => 8
export function cycleOffset(offset: number, total: number): number {
  return ((offset % total) + total) % total
}

// 计算全部可见槽位：槽位几何 + 滚轮偏移映射 + 贴靠边镜像
// Compute all visible slots: geometry + scroll mapping + attach-side mirroring
// calculateDockPositions(9, 0) => 9 个槽位，slotIndex 4 为选中位
// calculateDockPositions(9, 0) => 9 slots with slotIndex 4 selected
export function calculateDockPositions(
  count: number,
  offset: number,
  total: number = count,
  centerIndex: number = Math.floor(count / 2),
  side: DockSide = 'right'
): DockPosition[] {
  const positions: DockPosition[] = []
  for (let i = 0; i < count; i++) {
    const { x, y } = getPositionOnArc(i, count)
    // 防溢出：实际索引 = ((偏移 + i) % 总数 + 总数) % 总数
    // Prevent overflow: actualIndex = ((offset + i) % total + total) % total
    positions.push({
      // 左贴靠时按容器中轴镜像 x（基础几何恒为右贴靠）
      // Mirror x across the container axis when attached left (base geometry is always right)
      x: side === 'left' ? DOCK_WIDTH - x : x,
      y,
      slotIndex: i,
      actualIndex: cycleOffset(offset + i, total),
      isActive: i === centerIndex,
    })
  }
  return positions
}

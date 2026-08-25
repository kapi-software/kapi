// Dock 弧线数学单元测试
// Unit tests for the dock arc math
import { describe, it, expect } from 'vitest'
import {
  DOCK_WIDTH,
  DOCK_HEIGHT,
  getPositionOnArc,
  cycleOffset,
  calculateDockPositions,
} from '@/lib/dock-arc'

describe('弧线基础几何 / getPositionOnArc', () => {
  it('应正确计算 9 个插件的位置 / Should place 9 slots on the arc', () => {
    const positions = calculateDockPositions(9, 0)

    // 验证返回数组长度 / Verify array length
    expect(positions).toHaveLength(9)

    // 验证中心位置激活（centerIndex = ⌊9/2⌋ = 4）/ Center slot active
    expect(positions[4].isActive).toBe(true)
    expect(positions.filter((p) => p.isActive)).toHaveLength(1)

    // 验证坐标范围（容器 320×560 内）/ Coordinates inside the 320x560 container
    positions.forEach((pos) => {
      expect(pos.x).toBeGreaterThan(0)
      expect(pos.x).toBeLessThan(DOCK_WIDTH)
      expect(pos.y).toBeGreaterThan(0)
      expect(pos.y).toBeLessThan(DOCK_HEIGHT)
    })
  })

  it('选中位应在弧线最左点（视觉中心）/ The active slot sits at the arc apex', () => {
    // 9 槽位居中位 θ=π：x = 320−260 = 60, y = 280
    // The centered slot of 9 has θ=π: x = 320−260 = 60, y = 280
    const center = getPositionOnArc(4, 9)
    expect(center.x).toBeCloseTo(60, 0)
    expect(center.y).toBeCloseTo(280, 0)
  })

  it('两端槽位应对称 / End slots are symmetric', () => {
    const bottom = getPositionOnArc(0, 9)
    const top = getPositionOnArc(8, 9)
    expect(bottom.x).toBeCloseTo(top.x, 6)
    expect(bottom.y + top.y).toBeCloseTo(DOCK_HEIGHT, 0) // 关于 y=280 对称
  })

  it('单槽位守卫不除零 / Single-slot guard avoids division by zero', () => {
    expect(() => getPositionOnArc(0, 1)).not.toThrow()
    const only = getPositionOnArc(0, 1)
    // 单槽位固定在最左点 / Single slot pinned at the arc apex
    expect(only.x).toBeCloseTo(60, 0)
    expect(only.y).toBeCloseTo(280, 0)
  })
})

describe('滚轮循环 / cycleOffset', () => {
  it('正偏移取模 / Positive offsets wrap', () => {
    expect(cycleOffset(100, 9)).toBe(1) // 100 % 9 = 1
    expect(cycleOffset(0, 9)).toBe(0)
    expect(cycleOffset(9, 9)).toBe(0)
  })

  it('负偏移归一 / Negative offsets normalize', () => {
    expect(cycleOffset(-1, 9)).toBe(8)
    expect(cycleOffset(-10, 9)).toBe(8)
  })
})

describe('槽位映射 / calculateDockPositions', () => {
  it('偏移量超出范围时 actualIndex 正确回绕 / Out-of-range offsets wrap correctly', () => {
    const positions = calculateDockPositions(9, 100, 9)
    expect(positions[0].actualIndex).toBe(1) // 100 % 9 = 1
    expect(positions[8].actualIndex).toBe(0) // (100+8) % 9 = 0，循环回起点
  })

  it('负偏移同样回绕 / Negative offsets wrap too', () => {
    const positions = calculateDockPositions(9, -1, 9)
    expect(positions[0].actualIndex).toBe(8)
  })

  it('插件少于可见槽位时全部可见且不越界 / Fewer plugins than slots stay in range', () => {
    const positions = calculateDockPositions(9, 0, 3)
    expect(positions.map((p) => p.actualIndex)).toEqual([0, 1, 2, 0, 1, 2, 0, 1, 2])
  })

  it('应在 16ms 内完成计算（60fps）/ Should complete within 16ms (60fps)', () => {
    const start = performance.now()
    calculateDockPositions(9, 0)
    const duration = performance.now() - start
    expect(duration).toBeLessThan(16)
  })
})

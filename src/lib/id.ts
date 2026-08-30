// 短 ID 生成器：UUID 前 8 段足以区分工作流 / 节点 / 触发器等
// Short-id generator: first 8 hex chars of a UUID are enough for workflows / nodes / triggers
import { v4 as uuidv4 } from "uuid"

// 完整 UUID v4（去 dash），用于后端或需要全局唯一性的场景
// Full UUID v4 (no dashes) for cases requiring global uniqueness
export function uuid(): string {
  return uuidv4().replace(/-/g, "")
}

// 短 ID（截取前 8 字符）；前缀可选便于按类型辨识
// Short id (first 8 chars); optional prefix for type-aware ids
export function shortId(prefix?: string): string {
  const id = uuidv4().replace(/-/g, "").slice(0, 8)
  return prefix ? `${prefix}-${id}` : id
}
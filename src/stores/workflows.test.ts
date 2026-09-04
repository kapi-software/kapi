// 工作流 store 单测：验证 invoke 命令路径与广播副作用
// Workflow store unit tests: verify invoke command paths and the broadcast side effect
// 用 vi.mock 拦截 @tauri-apps/api 模块，浏览器 / Tauri 命令路径都能跑
// Mock the @tauri-apps/api module so both browser and Tauri paths run
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const invokeMock = vi.fn()
const emitMock = vi.fn()

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}))
vi.mock('@tauri-apps/api/event', () => ({
  emit: (...args: unknown[]) => emitMock(...args),
}))

import { useWorkflowsStore } from '@/stores/workflows'
import type { Workflow, WorkflowRun, WorkflowStepLog } from '@/types'

// 固定一次工作流 + 一次运行的 fixture，便于断言 invoke 参数
// Fix a workflow + run fixture so we can assert invoke params
const sampleWorkflow: Workflow = {
  id: 'wf-test-1',
  name: 'Sample',
  description: null,
  graph: { nodes: [], edges: [] },
  schema_version: 1,
  is_enabled: true,
  created_at: '',
  updated_at: '',
}

const sampleRun: WorkflowRun = {
  id: 42,
  workflow_id: 'wf-test-1',
  trigger_type: 'manual',
  status: 'success',
  error: null,
  started_at: '2026-08-30T10:00:00Z',
  finished_at: '2026-08-30T10:00:01Z',
}

const sampleSteps: WorkflowStepLog[] = [
  {
    id: 1,
    run_id: 42,
    step_id: 'n1',
    plugin_id: 'plugin.a',
    action: 'reverse',
    status: 'success',
    input: '{"text":"abc"}',
    output: '{"text":"cba"}',
    error: null,
    duration_ms: 12,
    created_at: '2026-08-30T10:00:00Z',
  },
]

describe('useWorkflowsStore / 工作流 store', () => {
  beforeEach(() => {
    invokeMock.mockReset()
    emitMock.mockReset().mockResolvedValue(undefined)
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('load 应调用 workflow_list / load invokes workflow_list', async () => {
    invokeMock.mockResolvedValueOnce([sampleWorkflow])
    const state = useWorkflowsStore.getState()
    await state.load()
    expect(invokeMock).toHaveBeenCalledWith('workflow_list')
    expect(useWorkflowsStore.getState().workflows).toEqual([sampleWorkflow])
  })

  it('save 应落库 + 刷新 + 广播 / save persists + reloads + broadcasts', async () => {
    invokeMock
      .mockResolvedValueOnce(undefined) // workflow_save
      .mockResolvedValueOnce([sampleWorkflow]) // workflow_list (reload)
    await useWorkflowsStore.getState().save(sampleWorkflow)
    expect(invokeMock.mock.calls[0]).toEqual(['workflow_save', { workflow: sampleWorkflow }])
    expect(invokeMock.mock.calls[1]).toEqual(['workflow_list'])
    expect(emitMock).toHaveBeenCalledWith('workflows:changed')
  })

  it('remove 应走 workflow_delete 并广播 / remove goes through workflow_delete and broadcasts', async () => {
    invokeMock
      .mockResolvedValueOnce(undefined) // workflow_delete
      .mockResolvedValueOnce([]) // workflow_list (reload)
    await useWorkflowsStore.getState().remove('wf-test-1')
    expect(invokeMock.mock.calls[0]).toEqual(['workflow_delete', { workflowId: 'wf-test-1' }])
    expect(emitMock).toHaveBeenCalledWith('workflows:changed')
  })

  it('run 应直接返回引擎产出的 run / run returns the engine-produced run', async () => {
    invokeMock.mockResolvedValueOnce(sampleRun)
    const result = await useWorkflowsStore.getState().run('wf-test-1')
    expect(result).toEqual(sampleRun)
    expect(invokeMock).toHaveBeenCalledWith('workflow_execute', { workflowId: 'wf-test-1' })
  })

  it('getRuns 应透传 limit / getRuns forwards limit', async () => {
    invokeMock.mockResolvedValueOnce([sampleRun])
    await useWorkflowsStore.getState().getRuns('wf-test-1', 5)
    expect(invokeMock).toHaveBeenCalledWith('workflow_runs', { workflowId: 'wf-test-1', limit: 5 })
  })

  it('getRunSteps 应以 run_id 调 workflow_run_steps / getRunSteps calls workflow_run_steps by runId', async () => {
    invokeMock.mockResolvedValueOnce(sampleSteps)
    const result = await useWorkflowsStore.getState().getRunSteps(42)
    expect(result).toEqual(sampleSteps)
    expect(invokeMock).toHaveBeenCalledWith('workflow_run_steps', { runId: 42 })
  })
})
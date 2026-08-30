// @kapi/plugin-sdk：插件前端桥接 SDK（单文件，宿主经 kapi-plugin:///__kapi__/sdk.js 分发）
// @kapi/plugin-sdk: the plugin-frontend bridge SDK (single file, served by the host at
// kapi-plugin:///__kapi__/sdk.js)
// 协议见 docs/PANEL.md §3：请求 {id, channel, payload}；响应 {id, ok, data|error}；
// 宿主推送 {kapiEvent: true, type, data, source}
// Protocol per docs/PANEL.md §3: request {id, channel, payload}; response {id, ok, data|error};
// host push {kapiEvent: true, type, data, source}
(function (global) {
  'use strict'

  var seq = 0
  var pending = new Map()
  var handlers = new Map() // type -> Set<fn>
  var typeRefs = new Map() // type -> 订阅计数（归零才真正退订） / refcount per type

  // ---- 宿主生命周期（uTools onPluginEnter/onPluginOut 同款语义）----
  // ---- host lifecycle (the same semantics as uTools onPluginEnter/onPluginOut) ----
  // enter：SDK 加载即向宿主查询环境并缓冲，注册即触发（迟注册也能拿到，只触发一次）
  // enter: queried from the host at load and buffered; fires on registration (late
  // registrations get it too) exactly once
  var enterState = { fired: false, payload: null }
  var enterCallbacks = []
  var leaveCallbacks = []

  function safeCall(fn, arg) {
    try {
      fn(arg)
    } catch (err) {
      // 插件回调抛错不影响其它回调 / one throwing callback never breaks the others
      setTimeout(function () { throw err }, 0)
    }
  }

  function fireEnter(payload) {
    if (enterState.fired) return
    enterState.fired = true
    enterState.payload = payload
    var cbs = enterCallbacks
    enterCallbacks = []
    cbs.forEach(function (fn) { safeCall(fn, payload) })
  }

  // 懒查询：首次注册 on.enter 才向宿主要环境（不用生命周期的插件零额外调用）
  // Lazy query: the environment is fetched only when on.enter is first registered
  // (plugins ignoring lifecycle pay no extra call)
  var enterRequested = false
  function requestEnter() {
    if (enterRequested || enterState.fired) return
    enterRequested = true
    call('kapi:window.getInfo', {}).then(
      function (info) { fireEnter(info || { mode: null }) },
      function () { fireEnter({ mode: null }) }
    )
  }

  function fireLeave() {
    var cbs = leaveCallbacks
    leaveCallbacks = []
    cbs.forEach(function (fn) { safeCall(fn) })
  }

  // 桥接错误：message 形如 "Code: detail"，code 供插件分支处理
  // Bridge error: message looks like "Code: detail"; .code is there for branching
  function BridgeError(message) {
    var err = new Error(message)
    var idx = message.indexOf(':')
    err.code = idx > 0 ? message.slice(0, idx) : 'BridgeError'
    err.name = 'BridgeError'
    return err
  }

  // 宿主判定：parent 存在、非自身、且可 postMessage（顶层页面 / 异常嵌入一律按无宿主）
  // Host detection: parent exists, isn't self and can postMessage (a top-level page or
  // an odd embedding counts as no host)
  function parentWindow() {
    if (
      global.parent &&
      global.parent !== global &&
      typeof global.parent.postMessage === 'function'
    ) {
      return global.parent
    }
    return null
  }

  // 裸 RPC：postMessage 到宿主页面，Promise 等待 {id, ok, ...} 响应
  // Raw RPC: postMessage to the host page, resolved by the {id, ok, ...} response
  function call(channel, payload) {
    var target = parentWindow()
    if (!target) {
      return Promise.reject(BridgeError('BridgeUnavailable: no plugin host (browser preview?)'))
    }
    return new Promise(function (resolve, reject) {
      var id = ++seq
      pending.set(id, { resolve: resolve, reject: reject })
      target.postMessage({ id: id, channel: channel, payload: payload == null ? {} : payload }, '*')
    })
  }

  // 宿主推送分发：仅认带 kapiEvent 标记的消息，不与 RPC 响应混淆
  // Host-push dispatch: only messages carrying the kapiEvent marker (never RPC responses)
  function onMessage(e) {
    var msg = e && e.data
    if (!msg || typeof msg !== 'object') return
    if (msg.kapiEvent === true) {
      var set = handlers.get(msg.type)
      if (set) {
        set.forEach(function (fn) {
          try {
            fn({ type: msg.type, data: msg.data == null ? null : msg.data, source: msg.source })
          } catch (err) {
            // 插件回调抛错不影响其它回调 / one throwing callback never breaks the others
            setTimeout(function () { throw err }, 0)
          }
        })
      }
      return
    }
    if (pending.has(msg.id)) {
      var entry = pending.get(msg.id)
      pending.delete(msg.id)
      if (msg.ok) entry.resolve(msg.data)
      else entry.reject(BridgeError(String(msg.error || 'bridge error')))
    }
  }

  if (typeof global.addEventListener === 'function') {
    global.addEventListener('message', onMessage, false)
  }

  // 页面卸载：先触发 leave 回调（内嵌返回列表 / 独立窗口关闭都走这条），再尽力退订
  // （嵌入视图被移除时宿主注册表不残留）
  // Page unload: fire leave callbacks first (embed-back-to-list and window close both
  // land here), then best-effort unsubscribe (no stale host entries)
  function releaseAll() {
    fireLeave()
    typeRefs.forEach(function (_count, type) {
      call('kapi:events.off', { type: type }).catch(function () {})
    })
    typeRefs.clear()
    handlers.clear()
  }
  if (typeof global.addEventListener === 'function') {
    global.addEventListener('pagehide', releaseAll, false)
  }

  global.kapi = {
    // 宿主 API 版本（对应 manifest 的 kapi_version）
    // Host API version (matches the manifest's kapi_version)
    version: '1.0.0',

    // 宿主生命周期回调（uTools onPluginEnter/onPluginOut 同款语义）
    // Host lifecycle callbacks (the same semantics as uTools onPluginEnter/onPluginOut)
    on: {
      // 进入插件视图时触发：callback({mode, ...})；mode = "embedded" | "independent" | null
      // Fires when the plugin view is entered: callback({mode}); late registrations
      // still receive it, exactly once per page load
      enter: function (callback) {
        if (typeof callback !== 'function') return
        if (enterState.fired) {
          // 已触发：微任务投递，保持异步语义一致
          // Already fired: deliver on a microtask to keep the async semantics uniform
          Promise.resolve().then(function () { safeCall(callback, enterState.payload) })
        } else {
          enterCallbacks.push(callback)
          requestEnter()
        }
      },
      // 离开插件视图时触发（内嵌返回列表 / 独立窗口关闭），无参数、只触发一次
      // Fires when the plugin view is left (embed back to list / window close);
      // no arguments, exactly once
      leave: function (callback) {
        if (typeof callback === 'function') leaveCallbacks.push(callback)
      },
    },

    storage: {
      // 读自身命名空间的键；返回 {value}（JSON 值或 null）
      // Read a key in the plugin's own namespace; returns {value} (JSON value or null)
      get: function (key) { return call('kapi:storage.get', { key: key }) },
      set: function (key, value) { return call('kapi:storage.set', { key: key, value: value }) },
      remove: function (key) { return call('kapi:storage.remove', { key: key }) },
    },

    clipboard: {
      readText: function () { return call('kapi:clipboard.read', {}) },
      writeText: function (text) { return call('kapi:clipboard.write', { text: text }) },
    },

    http: {
      // {url, method?, headers?, body?} → {status, headers, body}（域名需声明权限）
      // {url, method?, headers?, body?} -> {status, headers, body} (host permission required)
      fetch: function (options) { return call('kapi:http.fetch', options) },
    },

    log: {
      debug: function (message, data) { return call('kapi:log.debug', { message: message, data: data }) },
      info: function (message, data) { return call('kapi:log.info', { message: message, data: data }) },
      warn: function (message, data) { return call('kapi:log.warn', { message: message, data: data }) },
      error: function (message, data) { return call('kapi:log.error', { message: message, data: data }) },
    },

    window: {
      // 展示环境查询：{mode} = "embedded" | "independent"
      // Display context: {mode} = "embedded" | "independent"
      getInfo: function () { return call('kapi:window.getInfo', {}) },
      setTitle: function (title) { return call('kapi:window.setTitle', { title: title }) },
      // 内嵌语义：等效关闭插件页面；独立窗口：真正关窗
      // Embedded: closes the plugin page; independent window: closes the window
      close: function () { return call('kapi:window.close', {}) },
      minimize: function () { return call('kapi:window.minimize', {}) },
      startDragging: function () { return call('kapi:window.startDragging', {}) },
    },

    plugin: {
      // 调用自身 WASM 动作（无需权限声明）
      // Invoke the plugin's own WASM action (no permission declaration needed)
      invoke: function (action, payload) {
        return call('kapi:plugin.invoke', { action: action, payload: payload == null ? null : payload })
      },
    },

    events: {
      // 广播事件（写事件总线历史并扇出给订阅者）
      // Broadcast an event (appends to the bus history and fans out to subscribers)
      emit: function (type, data) { return call('kapi:events.emit', { type: type, data: data }) },

      // 订阅事件：resolve 出退订函数；同一 type 多个回调只占一条宿主订阅
      // Subscribe: resolves to an unsubscribe fn; N callbacks share one host subscription
      on: function (type, handler) {
        if (typeof handler !== 'function') {
          return Promise.reject(BridgeError('InvalidPayload: handler must be a function'))
        }
        if (!handlers.has(type)) {
          handlers.set(type, new Set())
          typeRefs.set(type, 0)
        }
        handlers.get(type).add(handler)
        typeRefs.set(type, typeRefs.get(type) + 1)
        // 回滚本地注册（订阅被拒时回调与计数不能残留）
        // Roll back the local registration (a denied subscribe must leave nothing behind)
        function rollback() {
          var set = handlers.get(type)
          if (set) set.delete(handler)
          var refs = (typeRefs.get(type) || 1) - 1
          if (refs > 0) {
            typeRefs.set(type, refs)
          } else {
            handlers.delete(type)
            typeRefs.delete(type)
          }
        }
        var alreadySubscribed = typeRefs.get(type) > 1
        var subscribe = alreadySubscribed
          ? Promise.resolve()
          : call('kapi:events.on', { type: type })
        return subscribe.then(
          function () {
            return function off() {
              if (!handlers.has(type) || !handlers.get(type).has(handler)) return
              rollback()
              if (!handlers.has(type)) {
                call('kapi:events.off', { type: type }).catch(function () {})
              }
            }
          },
          function (err) {
            rollback()
            throw err
          }
        )
      },
    },
  }
})(typeof window !== 'undefined' ? window : globalThis)

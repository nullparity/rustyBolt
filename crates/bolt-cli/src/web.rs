pub const HTML_PAGE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>rustyBolt — Settings</title>
  <link rel="icon" type="image/svg+xml" href="/icon.svg">
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Geist:wght@400;500;600;700&family=Geist+Mono:wght@400;500;600&display=swap" rel="stylesheet">
  <style>
    :root {
      --bg: #09090b;
      --card: #111114;
      --card-border: #232328;
      --card-highlight: rgba(255, 255, 255, 0.03);
      --input: #09090b;
      --input-border: #27272a;
      --input-focus: #52525b;
      --text: #fafafa;
      --text-muted: #a1a1aa;
      --text-subtle: #71717a;
      --copper: #d98f5c;
      --copper-dim: rgba(217, 143, 92, 0.15);
      --patina: #22a184;
      --patina-dim: rgba(34, 161, 132, 0.15);
      --red: #f43f5e;
      --font-sans: 'Geist', -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      --font-mono: 'Geist Mono', ui-monospace, SFMono-Regular, Menlo, monospace;
    }

    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      background-color: var(--bg);
      color: var(--text);
      font-family: var(--font-sans);
      min-height: 100vh;
      line-height: 1.5;
      padding-bottom: 96px;
      -webkit-font-smoothing: antialiased;
    }

    header {
      position: sticky;
      top: 0;
      z-index: 50;
      background: rgba(9, 9, 11, 0.85);
      backdrop-filter: blur(12px);
      -webkit-backdrop-filter: blur(12px);
      border-bottom: 1px solid var(--card-border);
      padding: 12px 24px;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }

    .brand {
      display: flex;
      align-items: center;
      gap: 10px;
    }
    .logo-img {
      width: 28px;
      height: 28px;
      border-radius: 6px;
      display: block;
    }
    h1 {
      font-size: 0.95rem;
      font-weight: 600;
      letter-spacing: -0.01em;
      color: var(--text);
    }
    .pill {
      display: inline-flex;
      align-items: center;
      gap: 5px;
      padding: 2px 8px;
      border-radius: 9999px;
      font-size: 0.75rem;
      font-weight: 500;
      border: 1px solid var(--input-border);
      background: rgba(255, 255, 255, 0.03);
      color: var(--text-muted);
    }
    .pill-dot {
      width: 6px;
      height: 6px;
      border-radius: 50%;
      background: var(--patina);
    }

    .header-actions {
      display: flex;
      align-items: center;
      gap: 10px;
    }
    .btn-ghost {
      background: transparent;
      border: 1px solid var(--input-border);
      color: var(--text-muted);
      padding: 5px 12px;
      border-radius: 6px;
      font-size: 0.8rem;
      font-weight: 500;
      cursor: pointer;
      transition: all 0.15s ease;
      font-family: inherit;
    }
    .btn-ghost:hover {
      background: rgba(255, 255, 255, 0.06);
      color: var(--text);
      border-color: var(--text-subtle);
    }

    main {
      max-width: 820px;
      margin: 32px auto;
      padding: 0 20px;
      display: flex;
      flex-direction: column;
      gap: 20px;
    }

    .card {
      background: var(--card);
      border: 1px solid var(--card-border);
      border-radius: 12px;
      padding: 20px 24px;
      box-shadow: inset 0 1px 0 0 var(--card-highlight), 0 4px 20px -4px rgba(0, 0, 0, 0.4);
    }

    .card-header {
      display: flex;
      align-items: flex-start;
      justify-content: space-between;
      margin-bottom: 18px;
    }
    .card-title-group {
      display: flex;
      flex-direction: column;
      gap: 3px;
    }
    .card-title {
      font-size: 0.95rem;
      font-weight: 600;
      letter-spacing: -0.01em;
      color: var(--text);
    }
    .card-desc {
      font-size: 0.8rem;
      color: var(--text-muted);
    }

    .client-list {
      display: flex;
      flex-direction: column;
      gap: 10px;
    }
    .client-item {
      display: flex;
      align-items: center;
      justify-content: space-between;
      background: #0e0e11;
      border: 1px solid var(--input-border);
      border-radius: 8px;
      padding: 10px 14px;
      gap: 12px;
    }
    .client-meta {
      display: flex;
      flex-direction: column;
      gap: 2px;
      min-width: 0;
    }
    .client-name {
      font-size: 0.85rem;
      font-weight: 600;
      color: var(--text);
    }
    .client-path {
      font-family: var(--font-mono);
      font-size: 0.75rem;
      color: var(--text-subtle);
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }

    .grid-2 {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
      gap: 16px;
    }

    .form-group {
      display: flex;
      flex-direction: column;
      gap: 6px;
      margin-bottom: 14px;
    }
    .form-group:last-child { margin-bottom: 0; }

    label {
      font-size: 0.8rem;
      font-weight: 500;
      color: var(--text-muted);
    }

    input[type="text"], select {
      background: var(--input);
      border: 1px solid var(--input-border);
      color: var(--text);
      padding: 8px 12px;
      border-radius: 6px;
      font-size: 0.85rem;
      font-family: inherit;
      outline: none;
      transition: border-color 0.15s ease, box-shadow 0.15s ease;
      width: 100%;
    }
    input[type="text"]:focus, select:focus {
      border-color: var(--input-focus);
      box-shadow: 0 0 0 1px var(--input-focus);
    }

    .font-mono {
      font-family: var(--font-mono) !important;
      font-size: 0.8rem !important;
    }

    .toggle-row {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 10px 0;
      border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    }
    .toggle-row:last-child { border-bottom: none; }
    .toggle-text {
      display: flex;
      flex-direction: column;
      gap: 2px;
    }
    .toggle-label {
      font-size: 0.85rem;
      font-weight: 500;
      color: var(--text);
    }
    .toggle-sub {
      font-size: 0.75rem;
      color: var(--text-subtle);
    }

    .switch {
      position: relative;
      display: inline-block;
      width: 36px;
      height: 20px;
      flex-shrink: 0;
    }
    .switch input { opacity: 0; width: 0; height: 0; }
    .slider {
      position: absolute;
      cursor: pointer;
      top: 0; left: 0; right: 0; bottom: 0;
      background-color: #27272a;
      transition: .2s cubic-bezier(0.4, 0, 0.2, 1);
      border-radius: 9999px;
    }
    .slider:before {
      position: absolute;
      content: "";
      height: 16px;
      width: 16px;
      left: 2px;
      bottom: 2px;
      background-color: #a1a1aa;
      transition: .2s cubic-bezier(0.4, 0, 0.2, 1);
      border-radius: 50%;
    }
    input:checked + .slider {
      background-color: var(--copper);
    }
    input:checked + .slider:before {
      transform: translateX(16px);
      background-color: #ffffff;
    }

    .segmented {
      display: flex;
      background: #18181b;
      border: 1px solid var(--input-border);
      border-radius: 8px;
      padding: 3px;
      gap: 3px;
    }
    .seg-btn {
      flex: 1;
      background: transparent;
      border: none;
      color: var(--text-muted);
      padding: 6px 10px;
      border-radius: 6px;
      font-size: 0.8rem;
      font-weight: 500;
      cursor: pointer;
      transition: all 0.15s ease;
      font-family: inherit;
      text-align: center;
    }
    .seg-btn:hover {
      color: var(--text);
    }
    .seg-btn.active {
      background: #27272a;
      color: #ffffff;
      box-shadow: 0 1px 3px rgba(0, 0, 0, 0.4);
    }

    .terminal-box {
      background: #000000;
      border: 1px solid var(--input-border);
      border-radius: 8px;
      padding: 14px 16px;
      font-family: var(--font-mono);
      font-size: 0.8rem;
      color: #e4e4e7;
      overflow-x: auto;
      white-space: pre-wrap;
      word-break: break-all;
      position: relative;
      line-height: 1.6;
    }
    .copy-btn {
      position: absolute;
      top: 10px;
      right: 10px;
      background: #18181b;
      border: 1px solid var(--input-border);
      color: var(--text-muted);
      padding: 4px 8px;
      border-radius: 6px;
      font-size: 0.75rem;
      cursor: pointer;
      display: inline-flex;
      align-items: center;
      gap: 4px;
      font-family: inherit;
      transition: all 0.15s ease;
    }
    .copy-btn:hover {
      background: #27272a;
      color: #fff;
    }
    .copy-btn svg {
      width: 12px;
      height: 12px;
    }

    .dock {
      position: fixed;
      bottom: 0;
      left: 0;
      right: 0;
      background: rgba(9, 9, 11, 0.85);
      backdrop-filter: blur(12px);
      -webkit-backdrop-filter: blur(12px);
      border-top: 1px solid var(--card-border);
      padding: 14px 24px;
      display: flex;
      align-items: center;
      justify-content: flex-end;
      gap: 16px;
      z-index: 40;
    }

    .btn-primary {
      background: #ffffff;
      color: #09090b;
      border: none;
      font-weight: 600;
      font-size: 0.85rem;
      padding: 8px 18px;
      border-radius: 6px;
      cursor: pointer;
      transition: all 0.15s ease;
      font-family: inherit;
    }
    .btn-primary:hover {
      background: #e4e4e7;
    }
    .btn-primary:active {
      transform: scale(0.98);
    }

    .toast {
      color: var(--patina);
      font-size: 0.8rem;
      font-weight: 500;
      opacity: 0;
      transition: opacity 0.2s ease;
    }
    .toast.show { opacity: 1; }

    .tag {
      display: inline-block;
      padding: 2px 7px;
      border-radius: 4px;
      font-size: 0.75rem;
      font-weight: 500;
      flex-shrink: 0;
    }
    .tag-ok {
      background: var(--patina-dim);
      color: var(--patina);
    }
    .tag-warn {
      background: var(--copper-dim);
      color: var(--copper);
    }
    .tag-err {
      background: rgba(244, 63, 94, 0.15);
      color: var(--red);
    }
    a.link {
      color: var(--copper);
      text-decoration: none;
    }
    a.link:hover {
      text-decoration: underline;
    }
  </style>
</head>
<body>
  <header>
    <div class="brand">
      <img class="logo-img" src="/icon.svg" alt="rustyBolt icon" width="28" height="28">
      <h1>rustyBolt</h1>
      <span class="pill"><span class="pill-dot"></span><span>127.0.0.1</span></span>
    </div>
    <div class="header-actions">
      <button class="btn-ghost" onclick="shutdownServer()">Close</button>
    </div>
  </header>

  <main>
    <div class="card">
      <div class="card-header">
        <div class="card-title-group">
          <div class="card-title">Java Runtime</div>
          <div class="card-desc">Runtime discovered on this machine. Feature 24 or newer unlocks modern JVM options.</div>
        </div>
      </div>
      
      <div class="form-group">
        <label for="java-select">Runtime Candidate</label>
        <select id="java-select" class="font-mono" onchange="onJavaSelectChange()">
          <option value="auto">Automatic (First Available)</option>
          <option value="custom">Custom Path...</option>
        </select>
      </div>

      <div class="form-group" id="java-custom-group" style="display:none;">
        <label for="java-custom-path">Binary Location</label>
        <input type="text" id="java-custom-path" class="font-mono" placeholder="/usr/bin/java or C:\Program Files\Java\bin\java.exe" oninput="markDirty()">
      </div>
    </div>

    <div class="card">
      <div class="card-header">
        <div class="card-title-group">
          <div class="card-title">Game Clients</div>
          <div class="card-desc">Installed game clients detected on this system.</div>
        </div>
      </div>
      
      <div class="client-list">
        <div class="client-item">
          <div class="client-meta">
            <span class="client-name">RuneLite</span>
            <span id="runelite-path" class="client-path">Checking...</span>
          </div>
          <span id="runelite-status" class="tag tag-warn">Checking</span>
        </div>

        <div class="client-item">
          <div class="client-meta">
            <span class="client-name">HDOS</span>
            <span id="hdos-path" class="client-path">Checking...</span>
          </div>
          <span id="hdos-status" class="tag tag-warn">Checking</span>
        </div>
      </div>

      <details style="margin-top: 12px;">
        <summary style="font-size: 0.75rem; color: var(--text-subtle); cursor: pointer; user-select: none;">Path overrides</summary>
        <div class="grid-2" style="margin-top: 10px; padding-top: 10px; border-top: 1px solid var(--card-border);">
          <div class="form-group">
            <label for="rl-custom-jar">RuneLite Jar Path</label>
            <input type="text" id="rl-custom-jar" class="font-mono" placeholder="Default detected path" oninput="markDirty()">
          </div>
          <div class="form-group">
            <label for="hdos-jar">HDOS Jar Path</label>
            <input type="text" id="hdos-jar" class="font-mono" placeholder="Default detected path" oninput="markDirty()">
          </div>
        </div>
      </details>
    </div>

    <div class="card">
      <div class="card-header">
        <div class="card-title-group">
          <div class="card-title">JVM Tuning</div>
          <div class="card-desc">Low-latency garbage collection and memory sizing for RuneLite.</div>
        </div>
        <label class="switch">
          <input type="checkbox" id="tuning-enabled" onchange="onToggleTuning()">
          <span class="slider"></span>
        </label>
      </div>

      <div id="tuning-body">
        <div class="form-group">
          <label>Garbage Collector</label>
          <div class="segmented">
            <button type="button" class="seg-btn" data-gc="z" onclick="setGc('z')">ZGC (Generational)</button>
            <button type="button" class="seg-btn" data-gc="g1" onclick="setGc('g1')">G1</button>
            <button type="button" class="seg-btn" data-gc="parallel" onclick="setGc('parallel')">Parallel</button>
            <button type="button" class="seg-btn" data-gc="default" onclick="setGc('default')">Default</button>
          </div>
        </div>

        <div class="grid-2" style="margin-top: 14px;">
          <div class="form-group">
            <label for="heap-min">Initial Heap (-Xms)</label>
            <input type="text" id="heap-min" class="font-mono" placeholder="2g" oninput="markDirty()">
          </div>
          <div class="form-group">
            <label for="heap-max">Maximum Heap (-Xmx)</label>
            <input type="text" id="heap-max" class="font-mono" placeholder="2g" oninput="markDirty()">
          </div>
        </div>

        <div class="grid-2" style="margin-top: 14px;">
          <div class="form-group">
            <label for="launch-mode">Launch Mode (--launch-mode)</label>
            <select id="launch-mode" onchange="markDirty()">
              <option value="reflect">Reflect (Recommended)</option>
              <option value="launcher">Launcher</option>
              <option value="auto">Automatic</option>
            </select>
          </div>
          <div class="form-group">
            <label for="hw-accel">Hardware Acceleration (--hw-accel)</label>
            <select id="hw-accel" onchange="markDirty()">
              <option value="metal">Metal (Recommended)</option>
              <option value="opengl">OpenGL</option>
              <option value="off">Off</option>
              <option value="auto">Automatic</option>
            </select>
          </div>
        </div>

        <div class="grid-2" style="margin-top: 14px;">
          <div class="form-group">
            <label for="direct-memory">Direct Memory (-XX:MaxDirectMemorySize)</label>
            <input type="text" id="direct-memory" class="font-mono" placeholder="512m" oninput="markDirty()">
          </div>
          <div class="form-group">
            <label for="metaspace">Metaspace (-XX:MaxMetaspaceSize)</label>
            <input type="text" id="metaspace" class="font-mono" placeholder="1g" oninput="markDirty()">
          </div>
        </div>

        <div class="grid-2" style="margin-top: 14px;">
          <div class="form-group">
            <label for="code-cache">Code Cache (-XX:ReservedCodeCacheSize)</label>
            <input type="text" id="code-cache" class="font-mono" placeholder="240m" oninput="markDirty()">
          </div>
          <div class="form-group">
            <label for="native-memory">Native Memory Tracking (-XX:NativeMemoryTracking)</label>
            <input type="text" id="native-memory" class="font-mono" placeholder="summary" oninput="markDirty()">
          </div>
        </div>

        <div style="margin-top: 12px; border-top: 1px solid rgba(255, 255, 255, 0.05); padding-top: 4px;">
          <div class="toggle-row">
            <div class="toggle-text">
              <span class="toggle-label">Compact Object Headers</span>
              <span class="toggle-sub">-XX:+UseCompactObjectHeaders (JDK 24+)</span>
            </div>
            <label class="switch">
              <input type="checkbox" id="compact-headers" onchange="markDirty()">
              <span class="slider"></span>
            </label>
          </div>
          <div class="toggle-row">
            <div class="toggle-text">
              <span class="toggle-label">String Deduplication</span>
              <span class="toggle-sub">-XX:+UseStringDeduplication</span>
            </div>
            <label class="switch">
              <input type="checkbox" id="string-dedup" onchange="markDirty()">
              <span class="slider"></span>
            </label>
          </div>
          <div class="toggle-row">
            <div class="toggle-text">
              <span class="toggle-label">Native Memory Access</span>
              <span class="toggle-sub">--enable-native-access</span>
            </div>
            <label class="switch">
              <input type="checkbox" id="native-access" onchange="markDirty()">
              <span class="slider"></span>
            </label>
          </div>
          <div class="toggle-row">
            <div class="toggle-text">
              <span class="toggle-label">Ahead-Of-Time Cache</span>
              <span class="toggle-sub">Pre-loads classes on subsequent launches</span>
            </div>
            <label class="switch">
              <input type="checkbox" id="aot-cache" onchange="markDirty()">
              <span class="slider"></span>
            </label>
          </div>
          <div class="toggle-row">
            <div class="toggle-text">
              <span class="toggle-label">GC Diagnostic Logging</span>
              <span class="toggle-sub">-Xlog:gc* into cache directory</span>
            </div>
            <label class="switch">
              <input type="checkbox" id="gc-log" onchange="markDirty()">
              <span class="slider"></span>
            </label>
          </div>
        </div>
      </div>
    </div>

    <div class="card">
      <div class="card-header">
        <div class="card-title-group">
          <div class="card-title">Home Directory Isolation</div>
          <div class="card-desc">Isolated mode keeps launcher settings distinct from ~/.runelite.</div>
        </div>
      </div>

      <div class="form-group">
        <label for="home-kind">Mode</label>
        <select id="home-kind" onchange="markDirty()">
          <option value="isolated">Isolated (Recommended)</option>
          <option value="system">System Default (~/.runelite)</option>
        </select>
      </div>
    </div>

    <div class="card">
      <div class="card-header">
        <div class="card-title-group">
          <div class="card-title">Launch Command Preview</div>
          <div class="card-desc">Live command generated for RuneLite based on active settings.</div>
        </div>
      </div>
      
      <div class="terminal-box">
        <button class="copy-btn" onclick="copyPreview()">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect width="14" height="14" x="8" y="8" rx="2" ry="2"></rect>
            <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path>
          </svg>
          <span id="copy-label">Copy</span>
        </button>
        <span id="preview-text">Loading command preview...</span>
      </div>
    </div>
  </main>

  <div class="dock">
    <span class="toast" id="toast">Saved</span>
    <button class="btn-primary" onclick="saveSettings()">Save Changes</button>
  </div>

  <script>
    let state = null;
    let selectedGc = 'z';

    async function loadState() {
      try {
        const res = await fetch('/api/state');
        state = await res.json();
        render();
      } catch (err) {
        console.error(err);
      }
    }

    function render() {
      if (!state) return;
      const config = state.config;

      const javaSelect = document.getElementById('java-select');
      javaSelect.innerHTML = `
        <option value="auto">Automatic (First Available)</option>
        <option value="custom">Custom Path...</option>
      `;
      state.runtimes.forEach((rt) => {
        const opt = document.createElement('option');
        opt.value = rt.path;
        opt.textContent = `${rt.path} (${rt.version || 'unknown'}, ${rt.source})`;
        javaSelect.appendChild(opt);
      });

      if (config.java_path) {
        let matched = false;
        for (let opt of javaSelect.options) {
          if (opt.value === config.java_path) {
            javaSelect.value = config.java_path;
            matched = true;
            break;
          }
        }
        if (!matched) {
          javaSelect.value = 'custom';
          document.getElementById('java-custom-path').value = config.java_path;
        }
      } else {
        javaSelect.value = 'auto';
      }
      onJavaSelectChange();

      const rlStatus = document.getElementById('runelite-status');
      const rlPath = document.getElementById('runelite-path');
      if (state.clients.runelite_detected) {
        rlStatus.className = 'tag tag-ok';
        rlStatus.textContent = 'Installed';
        rlPath.textContent = state.clients.runelite_detected;
      } else {
        rlStatus.className = 'tag tag-err';
        rlStatus.innerHTML = 'Not Found &middot; <a class="link" href="https://oldschool.runescape.wiki/w/RuneLite" target="_blank">Wiki</a>';
        rlPath.textContent = 'No RuneLite.jar detected';
      }

      const hdosStatus = document.getElementById('hdos-status');
      const hdosPath = document.getElementById('hdos-path');
      if (state.clients.hdos_detected) {
        hdosStatus.className = 'tag tag-ok';
        hdosStatus.textContent = 'Installed';
        hdosPath.textContent = state.clients.hdos_detected;
      } else {
        hdosStatus.className = 'tag tag-warn';
        hdosStatus.innerHTML = 'Not Found &middot; <a class="link" href="https://oldschool.runescape.wiki/w/HDOS" target="_blank">Wiki</a>';
        hdosPath.textContent = 'No hdos-launcher.jar detected';
      }

      document.getElementById('rl-custom-jar').value = config.runelite_custom_jar || '';
      document.getElementById('hdos-jar').value = config.hdos_jar || '';

      const tuning = config.runelite_tuning || {};
      const tuningEnabled = tuning.enabled !== false;
      document.getElementById('tuning-enabled').checked = tuningEnabled;
      document.getElementById('tuning-body').style.display = tuningEnabled ? 'block' : 'none';

      document.getElementById('heap-min').value = tuning.heap_min || '';
      document.getElementById('heap-max').value = tuning.heap_max || '';
      document.getElementById('launch-mode').value = tuning.launch_mode || 'reflect';
      document.getElementById('hw-accel').value = tuning.hw_accel || 'metal';
      document.getElementById('direct-memory').value = tuning.direct_memory_max || '';
      document.getElementById('metaspace').value = tuning.metaspace_max || '';
      document.getElementById('code-cache').value = tuning.code_cache_size || '';
      document.getElementById('native-memory').value = tuning.native_memory_tracking || '';
      document.getElementById('compact-headers').checked = !!tuning.compact_object_headers;
      document.getElementById('string-dedup').checked = !!tuning.string_deduplication;
      document.getElementById('native-access').checked = !!tuning.native_access;
      document.getElementById('aot-cache').checked = !!tuning.aot_cache;
      document.getElementById('gc-log').checked = !!tuning.gc_log;

      setGc(tuning.garbage_collector || 'z');

      const homeKind = document.getElementById('home-kind');
      homeKind.value = (config.runelite_home_kind === 'system') ? 'system' : 'isolated';

      document.getElementById('preview-text').textContent = state.runelite_plan || 'Ready to launch';
    }

    function onJavaSelectChange() {
      const val = document.getElementById('java-select').value;
      document.getElementById('java-custom-group').style.display = (val === 'custom') ? 'block' : 'none';
      markDirty();
    }

    function onToggleTuning() {
      const enabled = document.getElementById('tuning-enabled').checked;
      document.getElementById('tuning-body').style.display = enabled ? 'block' : 'none';
      markDirty();
    }

    function setGc(gc) {
      selectedGc = gc;
      document.querySelectorAll('.seg-btn').forEach(btn => {
        btn.classList.toggle('active', btn.dataset.gc === gc);
      });
      markDirty();
    }

    function markDirty() {
    }

    async function saveSettings() {
      if (!state) return;
      const config = state.config;

      const javaChoice = document.getElementById('java-select').value;
      if (javaChoice === 'auto') {
        config.java_path = null;
      } else if (javaChoice === 'custom') {
        const val = document.getElementById('java-custom-path').value.trim();
        config.java_path = val ? val : null;
      } else {
        config.java_path = javaChoice;
      }

      const customJar = document.getElementById('rl-custom-jar').value.trim();
      config.runelite_use_custom_jar = customJar.length > 0;
      config.runelite_custom_jar = customJar.length > 0 ? customJar : null;

      const hdosJar = document.getElementById('hdos-jar').value.trim();
      config.hdos_jar = hdosJar.length > 0 ? hdosJar : null;

      config.runelite_tuning = {
        enabled: document.getElementById('tuning-enabled').checked,
        heap_min: document.getElementById('heap-min').value.trim() || null,
        heap_max: document.getElementById('heap-max').value.trim() || null,
        stack_size: "2m",
        garbage_collector: selectedGc,
        compact_object_headers: document.getElementById('compact-headers').checked,
        string_deduplication: document.getElementById('string-dedup').checked,
        native_access: document.getElementById('native-access').checked,
        aot_cache: document.getElementById('aot-cache').checked,
        gc_log: document.getElementById('gc-log').checked,
        java2d_metal: true,
        launcher_nojvm: true,
        application_name: "RuneLite",
        process_name: "RuneLite",
        launch_mode: document.getElementById('launch-mode').value,
        hw_accel: document.getElementById('hw-accel').value,
        direct_memory_max: document.getElementById('direct-memory').value.trim() || null,
        metaspace_max: document.getElementById('metaspace').value.trim() || null,
        code_cache_size: document.getElementById('code-cache').value.trim() || null,
        native_memory_tracking: document.getElementById('native-memory').value.trim() || null,
        extra_jvm_args: [],
        extra_app_args: []
      };

      config.runelite_home_kind = document.getElementById('home-kind').value;

      try {
        const res = await fetch('/api/save', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(config)
        });
        const data = await res.json();
        if (data.ok) {
          if (data.plan) {
            document.getElementById('preview-text').textContent = data.plan;
          }
          const toast = document.getElementById('toast');
          toast.classList.add('show');
          setTimeout(() => toast.classList.remove('show'), 2000);
        }
      } catch (err) {
        alert('Failed to save settings: ' + err);
      }
    }

    async function shutdownServer() {
      try {
        await fetch('/api/shutdown', { method: 'POST' });
      } catch (e) {}
      window.close();
      document.body.innerHTML = `
        <div style="display:flex;align-items:center;justify-content:center;height:80vh;flex-direction:column;gap:12px;">
          <h2 style="font-size:1.1rem;font-weight:600;">Configuration server stopped</h2>
          <p style="color:var(--text-muted);font-size:0.85rem;">You may close this browser window.</p>
        </div>
      `;
    }

    function copyPreview() {
      const text = document.getElementById('preview-text').textContent;
      navigator.clipboard.writeText(text);
      const label = document.getElementById('copy-label');
      label.textContent = 'Copied';
      setTimeout(() => { label.textContent = 'Copy'; }, 1500);
    }

    window.addEventListener('DOMContentLoaded', loadState);
  </script>
</body>
</html>
"#;

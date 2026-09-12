pub const HTML_PAGE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>rustyBolt</title>
  <link rel="icon" type="image/svg+xml" href="/icon.svg">
  <style>
    :root {
      --bg: #131211;
      --surface: #1a1917;
      --surface-card: #201e1b;
      --surface-input: #151413;
      --border: #38332c;
      --border-focus: #c07440;
      --text: #f4ede6;
      --text-muted: #a89f93;
      --text-dim: #736b61;
      --accent: #d98f5c;
      --accent-glow: rgba(217, 143, 92, 0.25);
      --accent-hover: #e39a63;
      --patina: #22a184;
      --patina-light: #4fd9b6;
      --patina-glow: rgba(34, 161, 132, 0.25);
      --red: #d9534f;
      --font-mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
      --font-sans: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    }

    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      background: radial-gradient(circle at 50% 0%, #291e17 0%, var(--bg) 60%);
      color: var(--text);
      font-family: var(--font-sans);
      min-height: 100vh;
      line-height: 1.5;
      padding-bottom: 80px;
    }

    header {
      border-bottom: 1px solid var(--border);
      background: rgba(19, 18, 17, 0.92);
      backdrop-filter: blur(16px);
      position: sticky;
      top: 0;
      z-index: 50;
      padding: 14px 24px;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }

    .brand {
      display: flex;
      align-items: center;
      gap: 12px;
    }
    .logo-img {
      width: 36px;
      height: 36px;
      border-radius: 8px;
      display: block;
      box-shadow: 0 2px 8px rgba(0, 0, 0, 0.5);
    }
    h1 {
      font-size: 1.25rem;
      font-weight: 700;
      letter-spacing: -0.02em;
      color: var(--text);
    }
    .badge {
      background: rgba(217, 143, 92, 0.15);
      color: var(--accent);
      border: 1px solid rgba(217, 143, 92, 0.35);
      padding: 2px 8px;
      border-radius: 9999px;
      font-size: 0.75rem;
      font-weight: 600;
    }

    .header-actions {
      display: flex;
      align-items: center;
      gap: 12px;
    }

    .status-dot {
      display: inline-block;
      width: 8px;
      height: 8px;
      background: var(--patina-light);
      border-radius: 50%;
      box-shadow: 0 0 8px var(--patina-glow);
      margin-right: 6px;
    }

    .btn-secondary {
      background: rgba(255, 255, 255, 0.05);
      border: 1px solid var(--border);
      color: var(--text-muted);
      padding: 6px 14px;
      border-radius: 6px;
      font-size: 0.85rem;
      cursor: pointer;
      transition: all 0.15s ease;
    }
    .btn-secondary:hover {
      background: rgba(255, 255, 255, 0.1);
      color: var(--text);
    }

    main {
      max-width: 960px;
      margin: 32px auto;
      padding: 0 20px;
      display: flex;
      flex-direction: column;
      gap: 24px;
    }

    .card {
      background: var(--surface-card);
      border: 1px solid var(--border);
      border-radius: 12px;
      padding: 24px;
      box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
    }

    .card-title {
      font-size: 1.1rem;
      font-weight: 600;
      color: #fff;
      margin-bottom: 4px;
    }
    .card-subtitle {
      font-size: 0.85rem;
      color: var(--text-muted);
      margin-bottom: 20px;
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
      margin-bottom: 16px;
    }
    .form-group:last-child { margin-bottom: 0; }

    label {
      font-size: 0.85rem;
      font-weight: 500;
      color: var(--text-muted);
    }

    input[type="text"], select {
      background: var(--surface-input);
      border: 1px solid var(--border);
      color: var(--text);
      padding: 9px 12px;
      border-radius: 6px;
      font-size: 0.9rem;
      font-family: inherit;
      outline: none;
      transition: border-color 0.15s ease, box-shadow 0.15s ease;
      width: 100%;
    }
    input[type="text"]:focus, select:focus {
      border-color: var(--border-focus);
      box-shadow: 0 0 0 3px var(--accent-glow);
    }

    .select-runtime {
      font-family: var(--font-mono);
      font-size: 0.82rem;
    }

    .toggle-row {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 10px 0;
      border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    }
    .toggle-row:last-child { border-bottom: none; }
    .toggle-info {
      display: flex;
      flex-direction: column;
      gap: 2px;
    }
    .toggle-label {
      font-size: 0.9rem;
      font-weight: 500;
      color: var(--text);
    }
    .toggle-desc {
      font-size: 0.75rem;
      color: var(--text-dim);
    }

    .switch {
      position: relative;
      display: inline-block;
      width: 44px;
      height: 24px;
      flex-shrink: 0;
    }
    .switch input { opacity: 0; width: 0; height: 0; }
    .slider {
      position: absolute;
      cursor: pointer;
      top: 0; left: 0; right: 0; bottom: 0;
      background-color: rgba(255, 255, 255, 0.08);
      transition: .2s cubic-bezier(0.4, 0, 0.2, 1);
      border-radius: 24px;
      border: 1px solid var(--border);
    }
    .slider:before {
      position: absolute;
      content: "";
      height: 18px;
      width: 18px;
      left: 2px;
      bottom: 2px;
      background-color: #fff;
      transition: .2s cubic-bezier(0.4, 0, 0.2, 1);
      border-radius: 50%;
    }
    input:checked + .slider {
      background-color: var(--accent);
      border-color: var(--accent);
    }
    input:checked + .slider:before {
      transform: translateX(20px);
    }

    .gc-options {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
      gap: 8px;
      margin-top: 4px;
    }
    .gc-btn {
      background: rgba(255, 255, 255, 0.03);
      border: 1px solid var(--border);
      color: var(--text-muted);
      padding: 8px 12px;
      border-radius: 6px;
      font-size: 0.85rem;
      font-weight: 500;
      cursor: pointer;
      text-align: center;
      transition: all 0.15s ease;
    }
    .gc-btn.active {
      background: rgba(217, 143, 92, 0.15);
      border-color: var(--accent);
      color: var(--accent);
      box-shadow: 0 0 12px var(--accent-glow);
    }

    .preview-box {
      background: #0e0d0c;
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 14px;
      font-family: var(--font-mono);
      font-size: 0.82rem;
      color: #e39a63;
      overflow-x: auto;
      white-space: pre-wrap;
      word-break: break-all;
      position: relative;
    }
    .copy-btn {
      position: absolute;
      top: 8px;
      right: 8px;
      background: rgba(255, 255, 255, 0.08);
      border: 1px solid var(--border);
      color: var(--text-muted);
      padding: 4px 8px;
      border-radius: 4px;
      font-size: 0.75rem;
      cursor: pointer;
    }
    .copy-btn:hover { background: rgba(255, 255, 255, 0.18); color: #fff; }

    .save-bar {
      position: fixed;
      bottom: 0;
      left: 0;
      right: 0;
      background: rgba(19, 18, 17, 0.95);
      backdrop-filter: blur(16px);
      border-top: 1px solid var(--border);
      padding: 14px 24px;
      display: flex;
      align-items: center;
      justify-content: flex-end;
      gap: 16px;
      z-index: 40;
    }

    .save-btn {
      background: linear-gradient(135deg, var(--accent), var(--border-focus));
      border: none;
      color: #1a0f07;
      font-weight: 700;
      font-size: 0.95rem;
      padding: 10px 24px;
      border-radius: 8px;
      cursor: pointer;
      box-shadow: 0 2px 14px var(--accent-glow);
      transition: all 0.15s ease;
    }
    .save-btn:hover {
      transform: translateY(-1px);
      box-shadow: 0 4px 20px var(--accent-glow);
    }
    .toast {
      color: var(--patina-light);
      font-size: 0.85rem;
      font-weight: 600;
      opacity: 0;
      transition: opacity 0.2s ease;
    }
    .toast.show { opacity: 1; }

    .tag-green { color: var(--patina-light); font-weight: 600; font-size: 0.75rem; }
    .tag-yellow { color: var(--accent); font-weight: 600; font-size: 0.75rem; }
    .tag-red { color: var(--red); font-weight: 600; font-size: 0.75rem; }
    a.wiki-link { color: var(--accent); text-decoration: none; font-weight: 500; }
    a.wiki-link:hover { text-decoration: underline; }
  </style>
</head>
<body>
  <header>
    <div class="brand">
      <img class="logo-img" src="/icon.svg" alt="rustyBolt icon" width="36" height="36">
      <h1>rustyBolt</h1>
      <span class="badge">Configuration</span>
    </div>
    <div class="header-actions">
      <span><span class="status-dot"></span><span style="font-size:0.8rem; color:var(--text-muted)">Local Server</span></span>
      <button class="btn-secondary" onclick="shutdownServer()">Close</button>
    </div>
  </header>

  <main>
    <div class="card">
      <div class="card-title">Java Runtime Selection</div>
      <div class="card-subtitle">Select which Java JDK is used to run game clients. Feature 24 or newer unlocks modern JVM tuning.</div>
      
      <div class="form-group">
        <label for="java-select">Detected Runtimes</label>
        <select id="java-select" class="select-runtime" onchange="onJavaSelectChange()">
          <option value="auto">Automatic Discovery (Recommended)</option>
          <option value="custom">Custom Path...</option>
        </select>
      </div>

      <div class="form-group" id="java-custom-group" style="display:none;">
        <label for="java-custom-path">Custom Java Binary Path</label>
        <input type="text" id="java-custom-path" placeholder="/path/to/bin/java or C:\Path\To\java.exe" oninput="markDirty()">
      </div>
    </div>

    <div class="card">
      <div class="card-title">Game Clients</div>
      <div class="card-subtitle">Client detection and executable jar overrides for RuneLite and HDOS.</div>
      
      <div class="grid-2">
        <div style="border-right: 1px solid var(--border); padding-right: 16px;">
          <h3 style="font-size:0.95rem; margin-bottom: 8px;">RuneLite</h3>
          <div id="runelite-status" style="font-size:0.8rem; margin-bottom:12px;">Detecting...</div>

          <div class="toggle-row">
            <div class="toggle-info">
              <span class="toggle-label">Use Custom Jar</span>
              <span class="toggle-desc">Override detected RuneLite.jar</span>
            </div>
            <label class="switch">
              <input type="checkbox" id="rl-use-custom" onchange="onToggleCustomJar()">
              <span class="slider"></span>
            </label>
          </div>

          <div class="form-group" id="rl-custom-group" style="display:none; margin-top:8px;">
            <label for="rl-custom-jar">Custom Jar Path</label>
            <input type="text" id="rl-custom-jar" placeholder="/path/to/RuneLite.jar" oninput="markDirty()">
          </div>

          <div class="form-group" style="margin-top:12px;">
            <label for="rl-template">Launch Command Template (optional)</label>
            <input type="text" id="rl-template" placeholder="%command% (blank for default)" oninput="markDirty()">
          </div>
        </div>

        <div style="padding-left: 8px;">
          <h3 style="font-size:0.95rem; margin-bottom: 8px;">HDOS</h3>
          <div id="hdos-status" style="font-size:0.8rem; margin-bottom:12px;">Detecting...</div>

          <div class="form-group">
            <label for="hdos-jar">Custom Launcher Jar Path (optional)</label>
            <input type="text" id="hdos-jar" placeholder="/path/to/hdos-launcher.jar" oninput="markDirty()">
          </div>

          <div class="form-group">
            <label for="hdos-template">Launch Command Template (optional)</label>
            <input type="text" id="hdos-template" placeholder="%command% (blank for default)" oninput="markDirty()">
          </div>
        </div>
      </div>
    </div>

    <div class="card">
      <div class="card-title">JVM Tuning (RuneLite)</div>
      <div class="card-subtitle">Performance flags for frame pacing, reduced garbage collection pauses, and fast startup.</div>

      <div class="toggle-row" style="margin-bottom: 16px;">
        <div class="toggle-info">
          <span class="toggle-label">Enable JVM Performance Tuning</span>
          <span class="toggle-desc">Apply optimized memory limits and generational garbage collection</span>
        </div>
        <label class="switch">
          <input type="checkbox" id="tuning-enabled" onchange="markDirty()">
          <span class="slider"></span>
        </label>
      </div>

      <div class="form-group">
        <label>Garbage Collector</label>
        <div class="gc-options">
          <div class="gc-btn" data-gc="z" onclick="setGc('z')">ZGC Generational<br><span style="font-size:0.7rem; color:var(--patina-light)">Recommended</span></div>
          <div class="gc-btn" data-gc="g1" onclick="setGc('g1')">G1<br><span style="font-size:0.7rem; color:var(--text-dim)">Standard</span></div>
          <div class="gc-btn" data-gc="parallel" onclick="setGc('parallel')">Parallel<br><span style="font-size:0.7rem; color:var(--text-dim)">Throughput</span></div>
          <div class="gc-btn" data-gc="default" onclick="setGc('default')">JVM Default<br><span style="font-size:0.7rem; color:var(--text-dim)">Unchanged</span></div>
        </div>
      </div>

      <div class="grid-2" style="margin-top: 16px;">
        <div class="form-group">
          <label for="heap-min">Initial Heap (-Xms)</label>
          <input type="text" id="heap-min" placeholder="2g" oninput="markDirty()">
        </div>
        <div class="form-group">
          <label for="heap-max">Maximum Heap (-Xmx)</label>
          <input type="text" id="heap-max" placeholder="2g" oninput="markDirty()">
        </div>
      </div>

      <div class="form-group" style="margin-top: 12px;">
        <label style="margin-bottom: 8px;">Modern Java 24+ Optimizations</label>
        <div class="toggle-row">
          <div class="toggle-info">
            <span class="toggle-label">Compact Object Headers</span>
            <span class="toggle-desc">Shrinks Java object headers to 64-bit (-XX:+UseCompactObjectHeaders)</span>
          </div>
          <label class="switch">
            <input type="checkbox" id="compact-headers" onchange="markDirty()">
            <span class="slider"></span>
          </label>
        </div>
        <div class="toggle-row">
          <div class="toggle-info">
            <span class="toggle-label">String Deduplication</span>
            <span class="toggle-desc">Removes duplicate string objects in memory (-XX:+UseStringDeduplication)</span>
          </div>
          <label class="switch">
            <input type="checkbox" id="string-dedup" onchange="markDirty()">
            <span class="slider"></span>
          </label>
        </div>
        <div class="toggle-row">
          <div class="toggle-info">
            <span class="toggle-label">Native Memory Access</span>
            <span class="toggle-desc">Permits direct memory access without warnings (--enable-native-access)</span>
          </div>
          <label class="switch">
            <input type="checkbox" id="native-access" onchange="markDirty()">
            <span class="slider"></span>
          </label>
        </div>
        <div class="toggle-row">
          <div class="toggle-info">
            <span class="toggle-label">Ahead-Of-Time (AOT) Cache</span>
            <span class="toggle-desc">Pre-compiles classes on initial run for instantaneous subsequent launches</span>
          </div>
          <label class="switch">
            <input type="checkbox" id="aot-cache" onchange="markDirty()">
            <span class="slider"></span>
          </label>
        </div>
        <div class="toggle-row">
          <div class="toggle-info">
            <span class="toggle-label">GC Logging</span>
            <span class="toggle-desc">Write garbage collection log for diagnostics (-Xlog:gc*)</span>
          </div>
          <label class="switch">
            <input type="checkbox" id="gc-log" onchange="markDirty()">
            <span class="slider"></span>
          </label>
        </div>
      </div>
    </div>

    <div class="card">
      <div class="card-title">RuneLite Home Isolation</div>
      <div class="card-subtitle">Keep launcher profiles separate from other clients to avoid corrupting settings.</div>

      <div class="form-group">
        <label for="home-kind">Home Directory Mode</label>
        <select id="home-kind" onchange="markDirty()">
          <option value="isolated">Isolated (Default — protects ~/.runelite)</option>
          <option value="system">System (~/.runelite)</option>
        </select>
      </div>
    </div>

    <div class="card">
      <div class="card-title">Launch Command Preview</div>
      <div class="card-subtitle">The exact command line that rustybolt launch runelite will execute.</div>
      
      <div class="preview-box">
        <button class="copy-btn" onclick="copyPreview()">Copy</button>
        <code id="preview-text">Loading command line...</code>
      </div>
    </div>
  </main>

  <div class="save-bar">
    <span class="toast" id="toast">Settings saved successfully!</span>
    <button class="save-btn" onclick="saveSettings()">Save Changes</button>
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
        console.error('Failed to load state', err);
      }
    }

    function render() {
      if (!state) return;
      const config = state.config;

      const javaSelect = document.getElementById('java-select');
      javaSelect.innerHTML = `
        <option value="auto">Automatic Discovery (Recommended)</option>
        <option value="custom">Custom Path...</option>
      `;
      state.runtimes.forEach((rt) => {
        const opt = document.createElement('option');
        opt.value = rt.path;
        opt.textContent = `${rt.path} (${rt.version || 'feature unknown'}, ${rt.source})`;
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
      if (state.clients.runelite_detected) {
        rlStatus.innerHTML = `<span class="tag-green">Installed:</span> ${state.clients.runelite_detected}`;
      } else {
        rlStatus.innerHTML = `<span class="tag-red">Not Installed.</span> Install from <a class="wiki-link" href="https://oldschool.runescape.wiki/w/RuneLite" target="_blank">RuneLite Wiki</a>.`;
      }

      const hdosStatus = document.getElementById('hdos-status');
      if (state.clients.hdos_detected) {
        hdosStatus.innerHTML = `<span class="tag-green">Installed:</span> ${state.clients.hdos_detected}`;
      } else {
        hdosStatus.innerHTML = `<span class="tag-yellow">Not detected.</span> Install from <a class="wiki-link" href="https://oldschool.runescape.wiki/w/HDOS" target="_blank">HDOS Wiki</a>.`;
      }

      document.getElementById('rl-use-custom').checked = !!config.runelite_use_custom_jar;
      document.getElementById('rl-custom-jar').value = config.runelite_custom_jar || '';
      onToggleCustomJar();
      document.getElementById('rl-template').value = config.runelite_launch_command || '';

      document.getElementById('hdos-jar').value = config.hdos_jar || '';
      document.getElementById('hdos-template').value = config.hdos_launch_command || '';

      const tuning = config.runelite_tuning || {};
      document.getElementById('tuning-enabled').checked = tuning.enabled !== false;
      document.getElementById('heap-min').value = tuning.heap_min || '';
      document.getElementById('heap-max').value = tuning.heap_max || '';
      document.getElementById('compact-headers').checked = !!tuning.compact_object_headers;
      document.getElementById('string-dedup').checked = !!tuning.string_deduplication;
      document.getElementById('native-access').checked = !!tuning.native_access;
      document.getElementById('aot-cache').checked = !!tuning.aot_cache;
      document.getElementById('gc-log').checked = !!tuning.gc_log;

      setGc(tuning.garbage_collector || 'z');

      const homeKind = document.getElementById('home-kind');
      if (config.runelite_home_kind === 'system') {
        homeKind.value = 'system';
      } else {
        homeKind.value = 'isolated';
      }

      document.getElementById('preview-text').textContent = state.runelite_plan || 'Ready to launch';
    }

    function onJavaSelectChange() {
      const val = document.getElementById('java-select').value;
      document.getElementById('java-custom-group').style.display = (val === 'custom') ? 'block' : 'none';
      markDirty();
    }

    function onToggleCustomJar() {
      const checked = document.getElementById('rl-use-custom').checked;
      document.getElementById('rl-custom-group').style.display = checked ? 'block' : 'none';
      markDirty();
    }

    function setGc(gc) {
      selectedGc = gc;
      document.querySelectorAll('.gc-btn').forEach(btn => {
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

      config.runelite_use_custom_jar = document.getElementById('rl-use-custom').checked;
      const customJar = document.getElementById('rl-custom-jar').value.trim();
      config.runelite_custom_jar = customJar ? customJar : null;
      const rlTemplate = document.getElementById('rl-template').value.trim();
      config.runelite_launch_command = rlTemplate ? rlTemplate : null;

      const hdosJar = document.getElementById('hdos-jar').value.trim();
      config.hdos_jar = hdosJar ? hdosJar : null;
      const hdosTemplate = document.getElementById('hdos-template').value.trim();
      config.hdos_launch_command = hdosTemplate ? hdosTemplate : null;

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
          setTimeout(() => toast.classList.remove('show'), 2500);
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
          <h2>Configuration server stopped.</h2>
          <p style="color:var(--text-muted)">You can close this tab and return to the terminal.</p>
        </div>
      `;
    }

    function copyPreview() {
      const text = document.getElementById('preview-text').textContent;
      navigator.clipboard.writeText(text);
      const btn = document.querySelector('.copy-btn');
      btn.textContent = 'Copied!';
      setTimeout(() => { btn.textContent = 'Copy'; }, 1500);
    }

    window.addEventListener('DOMContentLoaded', loadState);
  </script>
</body>
</html>
"#;

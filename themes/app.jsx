// app.jsx — Lurker shell (stateless UI; only mounted-volume list is state)

const { useState, useEffect, useMemo } = React;

const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "theme": "dark",
  "backend": "linux",
  "density": "regular"
}/*EDITMODE-END*/;

// Seed: one volume already mounted, to demonstrate mount status
const initialVolumes = [
  {
    id: "v1",
    name: "archive",
    source: "~/vaults/archive.luks",
    mount: "/mnt/archive",
    cipher: "AES",
    readonly: false,
    sourceKind: "file",
  },
];

function App() {
  const [t, setTweak] = useTweaks(TWEAK_DEFAULTS);
  const [view, setView] = useState("manage");
  const [volumes, setVolumes] = useState(initialVolumes);
  const [toasts, setToasts] = useState([]);

  // When backend changes, fix the seed sample to match OS conventions
  useEffect(() => {
    setVolumes(vs => vs.map(v => {
      if (v.id !== "v1") return v;
      if (t.backend === "mac") {
        return { ...v, source: "~/vaults/archive.dmg", mount: "/Volumes/archive" };
      }
      return { ...v, source: "~/vaults/archive.luks", mount: "/mnt/archive" };
    }));
  }, [t.backend]);

  const pushToast = (toast) => {
    const id = Math.random().toString(36).slice(2);
    setToasts(ts => [...ts, { id, ...toast }]);
    setTimeout(() => setToasts(ts => ts.filter(x => x.id !== id)), 2800);
  };

  const onCreate = ({ target, size, cipher }) => {
    pushToast({ kind: "ok", t: "Container created", d: `${target}${size !== "—" ? " · " + size : ""}` });
    setView("manage");
  };

  const onMount = (v) => {
    setVolumes(vs => [v, ...vs]);
    pushToast({ kind: "ok", t: "Volume mounted", d: `${v.name} → ${v.mount}` });
  };

  const onUnmount = (id) => {
    const v = volumes.find(x => x.id === id);
    if (!v) return;
    setVolumes(vs => vs.filter(x => x.id !== id));
    pushToast({ kind: "info", t: "Volume unmounted", d: `${v.name} · locked` });
  };

  useEffect(() => {
    const onKey = (e) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "1") { e.preventDefault(); setView("create"); }
      if ((e.metaKey || e.ctrlKey) && e.key === "2") { e.preventDefault(); setView("manage"); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const isLinux = t.backend === "linux";
  const title = {
    create:  "Create encrypted container",
    manage:  "Manage volumes",
  }[view];
  const subtitle = {
    create:  "Make a new encrypted file or partition.",
    manage:  "Mounted volumes, and unlock new ones.",
  }[view];

  const rootStyle = {
    "--density": t.density === "compact" ? 0.88 : t.density === "comfy" ? 1.12 : 1,
  };

  return (
    <div className="app" style={rootStyle} data-theme={t.theme}>
      <div className="titlebar">
        <div className="tb-left">
          <span className="tb-dot"/>
          <span className="tb-title">Lurker</span>
          <span className="tb-status">— {volumes.length} mounted · {isLinux ? "Linux" : "macOS"}</span>
        </div>
        <div className="tb-ctrls">
          <button className="tb-btn"><Icon.Min/></button>
          <button className="tb-btn"><Icon.Max/></button>
          <button className="tb-btn close"><Icon.CloseW/></button>
        </div>
      </div>

      <div className="body">
        <aside className="side">
          <div className="brand">
            <div className="brand-mark">
              <div className="brand-glyph"><Icon.Shield/></div>
              <div style={{display:"flex",flexDirection:"column"}}>
                <span className="brand-name">Lurker</span>
                <span className="brand-tag">{isLinux ? "LUKS · VeraCrypt" : "Apple · VeraCrypt"}</span>
              </div>
            </div>
          </div>

          <div className="nav-label">Actions</div>
          <div className="nav-group">
            <NavItem active={view==="create"} onClick={() => setView("create")} icon={<Icon.Plus/>}     kbd="⌘1" label="Create"/>
            <NavItem active={view==="manage"} onClick={() => setView("manage")} icon={<Icon.LockOpen/>} kbd="⌘2" label="Manage"/>
          </div>

        </aside>

        <main className="main">
          <div className="main-hd">
            <div style={{flex:1,minWidth:0}}>
              <h1 className="main-title">{title}</h1>
              <p className="main-sub">{subtitle}</p>
            </div>
            <div className="actions">
              <button
                className="icon-round"
                title={`Switch to ${t.theme === "dark" ? "light" : "dark"} mode`}
                onClick={() => setTweak("theme", t.theme === "dark" ? "light" : "dark")}
                aria-label={t.theme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
              >
                {t.theme === "dark" ? <Icon.Sun/> : <Icon.Moon/>}
              </button>
            </div>
          </div>

          <div className="main-scroll">
            <div style={{position:"relative"}}>
              <div className="glow"/>
              {view === "create" && <CreateView backend={t.backend} onCreate={onCreate}/>}
              {view === "manage" && <ManageView backend={t.backend} volumes={volumes} onMount={onMount} onUnmount={onUnmount}/>}
            </div>
          </div>
        </main>
      </div>

      <div className="toasts">
        {toasts.map(x => (
          <div key={x.id} className="toast" data-kind={x.kind}>
            <span className="dot"/>
            <div style={{display:"flex",flexDirection:"column",gap:2}}>
              <span className="t">{x.t}</span>
              <span className="d">{x.d}</span>
            </div>
          </div>
        ))}
      </div>

      <TweaksPanel>
        <TweakSection label="Theme"/>
        <TweakRadio label="Mode" value={t.theme}
          options={["dark","light"]}
          onChange={v => setTweak("theme", v)}/>
        <TweakSection label="Backend"/>
        <TweakRadio label="OS" value={t.backend}
          options={["linux","mac"]}
          onChange={v => setTweak("backend", v)}/>
        <TweakSection label="Density"/>
        <TweakRadio label="Size" value={t.density}
          options={["compact","regular","comfy"]}
          onChange={v => setTweak("density", v)}/>
      </TweaksPanel>
    </div>
  );
}

function NavItem({ icon, label, kbd, active, onClick }) {
  return (
    <div className="nav-item" data-active={!!active} onClick={onClick}>
      <span className="nav-ico">{icon}</span>
      <span>{label}</span>
      {kbd && <span className="nav-kbd">{kbd}</span>}
    </div>
  );
}

function VolCard({ v, selectable, selected, onClick }) {
  return (
    <div className="vol" data-selected={selected} onClick={onClick}>
      <div className="vol-top">
        <div className="vol-icon"><Icon.LockOpen/></div>
        <div className="vol-name">{v.name}</div>
        <div className="vol-state" style={{color: v.readonly ? "var(--warn)" : "var(--ok)"}}>
          <span className="dot"/>
          {v.readonly ? "RO" : "RW"}
        </div>
      </div>
      <div className="vol-meta">
        <div><b>source</b>{v.source}</div>
        <div><b>mount</b>{v.mount}</div>
        {v.cipher && <div><b>cipher</b>{v.cipher}</div>}
      </div>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App/>);

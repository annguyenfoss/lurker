// views.jsx — Create / Mount / Unmount flow bodies
// Stateless UI. The only stateful thing globally is the mounted-volume list.
const { useState, useEffect, useMemo, useRef } = React;

/* ─── Shared primitives ─────────────────────────────────────────── */

function Field({ label, hint, required, children }) {
  return (
    <div className="field">
      <div className="lbl">
        <span className="lbl-t">{label}{required && <span className="lbl-req">required</span>}</span>
        {hint && <span className="lbl-h">{hint}</span>}
      </div>
      {children}
    </div>
  );
}

function Segmented({ value, onChange, options }) {
  return (
    <div className="seg">
      {options.map(o => (
        <button key={o.value} data-on={value === o.value} onClick={() => onChange(o.value)}>{o.label}</button>
      ))}
    </div>
  );
}

function Toggle({ label, hint, value, onChange }) {
  return (
    <label style={{display:"flex",alignItems:"center",gap:12,cursor:"default"}} onClick={()=>onChange(!value)}>
      <span style={{
        width:32,height:18,borderRadius:999,
        background: value ? "var(--accent)" : "var(--bg-3)",
        border:".5px solid var(--line-strong)",
        position:"relative",flex:"none",transition:"background .15s"
      }}>
        <span style={{
          position:"absolute",top:1.5,left: value?15:1.5,
          width:13,height:13,borderRadius:999,
          background:"var(--bg-0)",
          transition:"left .15s",
          boxShadow:"0 1px 3px rgba(0,0,0,.5)"
        }}/>
      </span>
      <span style={{display:"flex",flexDirection:"column",gap:2}}>
        <span style={{fontSize:12.5,fontWeight:500,color:"var(--fg-1)"}}>{label}</span>
        {hint && <span style={{fontSize:11,color:"var(--fg-3)",fontFamily:"var(--mono)"}}>{hint}</span>}
      </span>
    </label>
  );
}

function PassphraseInput({ value, onChange, placeholder = "enter a strong passphrase", showStrength = true }) {
  const [show, setShow] = useState(false);
  const score = useMemo(() => {
    let s = 0;
    if (value.length >= 8) s++;
    if (value.length >= 14) s++;
    if (/[A-Z]/.test(value) && /[a-z]/.test(value)) s++;
    if (/\d/.test(value) && /[^\w\s]/.test(value)) s++;
    return s;
  }, [value]);
  return (
    <div>
      <div className="inp-wrap">
        <input
          className="inp"
          type={show ? "text" : "password"}
          value={value}
          placeholder={placeholder}
          onChange={e => onChange(e.target.value)}
          spellCheck={false}
          autoComplete="off"
        />
        <div className="suffix">
          <button className="icon-btn" title={show ? "Hide" : "Show"} onClick={() => setShow(s => !s)}>
            {show ? <Icon.EyeOff/> : <Icon.Eye/>}
          </button>
          {showStrength && (
            <button className="icon-btn" title="Generate diceware"
              onClick={() => {
                const words = ["forest","ember","harbor","gravel","lucid","orbit","thistle","wren","basalt","marrow","quartz","syntax"];
                const p = Array.from({length:5},()=>words[Math.floor(Math.random()*words.length)]).join("-") + "-" + Math.floor(Math.random()*99);
                onChange(p);
              }}>
              <Icon.Dice/>
            </button>
          )}
        </div>
      </div>

    </div>
  );
}

function KeyFilePicker({ value, onChange }) {
  return value ? (
    <div className="inp-wrap">
      <span className="prefix"><Icon.Key/></span>
      <input className="inp has-prefix" style={{paddingLeft:30}} value={value} onChange={e => onChange(e.target.value)} spellCheck={false}/>
      <div className="suffix">
        <button className="icon-btn" title="Clear" onClick={() => onChange("")}><Icon.X/></button>
      </div>
    </div>
  ) : (
    <div
      className="drop"
      style={{padding:"18px 16px"}}
      onClick={() => onChange("~/.lurker/keys/archive.key")}
    >
      <Icon.Key/>
      <div className="drop-t">Select a key file</div>
      <div className="drop-s">drop a file here or click to browse</div>
    </div>
  );
}

/* Auth block used by Mount & Unmount: toggle between passphrase and keyfile */
function AuthBlock({ method, onMethod, pass, onPass, keyfile, onKey, showStrength = true }) {
  return (
    <div>
      <div style={{marginBottom:10}}>
        <Segmented value={method} onChange={onMethod} options={[
          {value:"pass", label:"Passphrase"},
          {value:"key",  label:"Key file"},
        ]}/>
      </div>
      {method === "pass" ? (
        <PassphraseInput value={pass} onChange={onPass} showStrength={showStrength}/>
      ) : (
        <KeyFilePicker value={keyfile} onChange={onKey}/>
      )}
    </div>
  );
}

/* ─── CREATE ────────────────────────────────────────────────────── */

function CreateView({ backend, onCreate }) {
  const isLinux = backend === "linux";
  const [targetKind, setTargetKind] = useState("file"); // "file" | "partition"
  const [format, setFormat] = useState(isLinux ? "luks" : "vera"); // linux: luks|vera ; mac: vera|apple
  const [filePath, setFilePath] = useState("~/vaults/");
  const [fileName, setFileName] = useState(isLinux ? "archive.luks" : "archive.dmg");
  const [partition, setPartition] = useState(isLinux ? "/dev/sdb1" : "/dev/disk2s1");
  const [size, setSize] = useState("1.0");
  const [sizeUnit, setSizeUnit] = useState("GB");
  const [cipher, setCipher] = useState("aes");
  const [pass, setPass] = useState("");
  const [confirm, setConfirm] = useState("");

  useEffect(() => {
    setFormat(isLinux ? "luks" : "vera");
    setFileName(f => {
      if (isLinux && f.endsWith(".dmg")) return "archive.luks";
      if (!isLinux && f.endsWith(".luks")) return "archive.dmg";
      return f;
    });
    setPartition(isLinux ? "/dev/sdb1" : "/dev/disk2s1");
  }, [backend]);

  const full = targetKind === "file"
    ? filePath.replace(/\/?$/, "/") + fileName
    : partition;

  const passMatch = pass.length > 0 && pass === confirm;
  const canSubmit = (targetKind === "file" ? fileName.trim() : partition.trim()) && pass.length >= 8 && passMatch;

  const sizeLabel = `${size} ${sizeUnit}`;

  return (
    <div className="card">
      <div className="card-section">
        <div className="sec-head">
          <div className="sec-title">Format</div>
          <div className="sec-hint">first is default</div>
        </div>
        <Segmented value={format} onChange={setFormat} options={isLinux
          ? [{value:"luks",label:"LUKS"},{value:"vera",label:"VeraCrypt"}]
          : [{value:"vera",label:"VeraCrypt"},{value:"apple",label:"Apple Encryption"}]
        }/>
      </div>

      <div className="card-section">
        <div className="sec-head">
          <div className="sec-title">Target</div>
          <div className="sec-hint">file or partition</div>
        </div>
        <div style={{marginBottom:"var(--s3)"}}>
          <Segmented value={targetKind} onChange={setTargetKind} options={[
            {value:"file",      label:"File container"},
            {value:"partition", label:"Partition / disk"},
          ]}/>
        </div>

        {targetKind === "file" ? (
          <>
            <Field label="Location" required>
              <div className="inp-wrap">
                <input className="inp" value={filePath} onChange={e => setFilePath(e.target.value)} spellCheck={false}/>
                <div className="suffix"><button className="icon-btn" title="Pick folder"><Icon.Folder/></button></div>
              </div>
            </Field>
            <div className="field-row">
              <Field label="Filename" required>
                <input className="inp" style={{width:"100%",minWidth:0}} value={fileName} onChange={e => setFileName(e.target.value)} spellCheck={false}/>
              </Field>
              <Field label="Size" required>
                <div style={{display:"flex",gap:"var(--s2)",width:"100%",minWidth:0}}>
                  <input className="inp" style={{flex:"1 1 0",minWidth:0,width:"100%"}} type="text" inputMode="decimal"
                    value={size} onChange={e => setSize(e.target.value)} spellCheck={false}/>
                  <select className="sel" style={{width:80,flex:"0 0 80px"}} value={sizeUnit} onChange={e => setSizeUnit(e.target.value)}>
                    <option value="MB">MB</option>
                    <option value="GB">GB</option>
                  </select>
                </div>
              </Field>
            </div>
          </>
        ) : (
          <>
            <Field label="Partition" required>
              <div className="inp-wrap">
                <span className="prefix"><Icon.Drive/></span>
                <input className="inp has-prefix" style={{paddingLeft:30}} value={partition} onChange={e => setPartition(e.target.value)} spellCheck={false}/>
                <div className="suffix"><button className="icon-btn" title="List devices"><Icon.Refresh/></button></div>
              </div>
            </Field>
            <div style={{
              display:"flex",gap:10,padding:"10px 12px",
              background:"oklch(58% 0.18 25 / .1)",borderRadius:"var(--radius-sm)",
              border:".5px solid oklch(58% 0.18 25 / .35)",fontSize:11.5,color:"var(--fg-1)",
              fontFamily:"var(--mono)",lineHeight:1.5,marginTop:"var(--s3)"
            }}>
              <span style={{color:"var(--danger)",flex:"none"}}><Icon.Info/></span>
              <span>All data on <b style={{color:"var(--fg-0)"}}>{partition}</b> will be erased.</span>
            </div>
          </>
        )}
      </div>

      <div className="card-section">
        <div className="sec-head">
          <div className="sec-title">Cipher</div>
          <div className="sec-hint">pick one — the rest is handled for you</div>
        </div>
        <Segmented value={cipher} onChange={setCipher} options={[
          {value:"aes",     label:"AES"},
          {value:"serpent", label:"Serpent"},
          {value:"twofish", label:"Twofish"},
        ]}/>
      </div>

      <div className="card-section">
        <div className="sec-head">
          <div className="sec-title">Passphrase</div>
          <div className="sec-hint">min 8 characters</div>
        </div>
        <Field label="Passphrase" required>
          <PassphraseInput value={pass} onChange={setPass}/>
        </Field>
        <Field label="Confirm">
          <div className="inp-wrap">
            <input
              className="inp"
              type="password"
              value={confirm}
              onChange={e => setConfirm(e.target.value)}
              placeholder="retype to confirm"
              spellCheck={false}
              autoComplete="off"
            />
            {confirm.length > 0 && (
              <div className="suffix">
                <span style={{
                  fontSize:10.5,fontFamily:"var(--mono)",padding:"0 8px",
                  color: passMatch ? "var(--ok)" : "var(--danger)"
                }}>{passMatch ? "matches" : "mismatch"}</span>
              </div>
            )}
          </div>
        </Field>
      </div>

      <div className="card-foot">
        <div className="foot-hint"></div>
        <div className="foot-actions">
          <button className="btn btn-primary" disabled={!canSubmit} onClick={() => {
            onCreate({ target: full, kind: targetKind, size: targetKind === "file" ? sizeLabel : "—", cipher });
            setPass(""); setConfirm("");
          }}>
            <Icon.Plus/> Create <span className="btn-kbd">⌘⏎</span>
          </button>
        </div>
      </div>
    </div>
  );
}



/* ─── MOUNT ────────────────────────────────────────────────────── */

function MountView({ backend, onMount }) {
  const isLinux = backend === "linux";
  const [sourceKind, setSourceKind] = useState("file"); // "file" | "partition"
  const [source, setSource] = useState("");
  const [mount, setMount] = useState(isLinux ? "/mnt/" : "/Volumes/");
  const [method, setMethod] = useState("pass");
  const [pass, setPass] = useState("");
  const [keyfile, setKeyfile] = useState("");
  const [readonly, setReadonly] = useState(false);
  const [drag, setDrag] = useState(false);

  const hasAuth = method === "pass" ? pass.length > 0 : keyfile.length > 0;
  const canSubmit = source && hasAuth && mount.replace(/\/?$/, "") !== (isLinux ? "/mnt" : "/Volumes");

  useEffect(() => {
    // adjust default mount base when backend swaps
    setMount(m => {
      if (isLinux && m.startsWith("/Volumes")) return "/mnt/";
      if (!isLinux && m.startsWith("/mnt")) return "/Volumes/";
      return m;
    });
  }, [backend]);

  return (
    <div className="card">
      <div className="card-section">
        <div className="sec-head">
          <div className="sec-title">Source</div>
          <div className="sec-hint">encrypted file or partition to unlock</div>
        </div>

        <div style={{marginBottom:"var(--s3)"}}>
          <Segmented value={sourceKind} onChange={v => { setSourceKind(v); setSource(""); }} options={[
            {value:"file",      label:"File container"},
            {value:"partition", label:"Partition / disk"},
          ]}/>
        </div>

        {sourceKind === "file" ? (
          !source ? (
            <div
              className="drop"
              data-drag={drag}
              onDragOver={e => { e.preventDefault(); setDrag(true); }}
              onDragLeave={() => setDrag(false)}
              onDrop={e => { e.preventDefault(); setDrag(false); const f = e.dataTransfer.files[0]; if (f) setSource(f.name.startsWith("/")?f.name:`~/${f.name}`); }}
              onClick={() => setSource(isLinux ? "~/vaults/archive.luks" : "~/vaults/archive.dmg")}
            >
              <Icon.File/>
              <div className="drop-t">Drop a container file here</div>
              <div className="drop-s">or click to browse · LUKS, VeraCrypt{!isLinux && ", DMG"}</div>
            </div>
          ) : (
            <div className="inp-wrap">
              <span className="prefix"><Icon.File/></span>
              <input className="inp has-prefix" style={{paddingLeft:30}} value={source} onChange={e => setSource(e.target.value)}/>
              <div className="suffix">
                <button className="icon-btn" title="Clear" onClick={() => setSource("")}><Icon.X/></button>
              </div>
            </div>
          )
        ) : (
          <div className="inp-wrap">
            <span className="prefix"><Icon.Drive/></span>
            <input
              className="inp has-prefix"
              style={{paddingLeft:30}}
              value={source}
              placeholder={isLinux ? "/dev/sdb1" : "/dev/disk2s1"}
              onChange={e => setSource(e.target.value)}
            />
            <div className="suffix"><button className="icon-btn" title="List devices"><Icon.Refresh/></button></div>
          </div>
        )}
      </div>

      <div className="card-section">
        <div className="sec-head">
          <div className="sec-title">Unlock with</div>
          <div className="sec-hint">passphrase or key file</div>
        </div>
        <AuthBlock
          method={method} onMethod={setMethod}
          pass={pass} onPass={setPass}
          keyfile={keyfile} onKey={setKeyfile}
          showStrength={false}
        />
      </div>

      <div className="card-section">
        <div className="sec-head"><div className="sec-title">Mount</div></div>
        <Field label="Mount point" hint={isLinux ? "/mnt/…" : "/Volumes/…"} required>
          <input className="inp" value={mount} onChange={e => setMount(e.target.value)}/>
        </Field>
        <Toggle label="Mount read-only" hint="safe for inspection · prevents writes" value={readonly} onChange={setReadonly}/>
      </div>

      <div className="card-foot">
        <div className="foot-hint"></div>
        <div className="foot-actions">
          <button className="btn btn-ghost">Reset</button>
          <button className="btn btn-primary" disabled={!canSubmit} onClick={() => {
            const baseName = source.split("/").pop().replace(/\.[^.]+$/,"") || "volume";
            onMount({
              id: "v" + Math.random().toString(36).slice(2,6),
              name: baseName,
              source, mount, readonly,
              method,
              sourceKind,
            });
            setPass(""); setKeyfile("");
          }}>
            <Icon.LockOpen/> Unlock &amp; mount <span className="btn-kbd">⌘⏎</span>
          </button>
        </div>
      </div>
    </div>
  );
}

/* ─── UNMOUNT ──────────────────────────────────────────────────── */

function UnmountView({ volumes, selectedId, onSelect, onUnmount, backend }) {
  const sel = volumes.find(v => v.id === selectedId);

  if (volumes.length === 0) {
    return (
      <div className="card">
        <div className="card-section" style={{padding:"var(--s6) var(--s5)",textAlign:"center"}}>
          <div style={{
            width:48,height:48,borderRadius:12,margin:"0 auto var(--s3)",
            background:"var(--bg-2)",display:"grid",placeItems:"center",color:"var(--fg-3)"
          }}>
            <Icon.Lock/>
          </div>
          <div style={{fontSize:15,fontWeight:600,color:"var(--fg-0)",marginBottom:6}}>Nothing to unmount</div>
          <div style={{fontSize:12.5,color:"var(--fg-2)",maxWidth:"40ch",margin:"0 auto",lineHeight:1.5}}>
            No volumes are currently mounted. Open a container from the <b style={{color:"var(--fg-1)"}}>Mount</b> tab to get started.
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="card">
      <div className="card-section">
        <div className="sec-head">
          <div className="sec-title">Pick a volume</div>
          <div className="sec-hint">{volumes.length} mounted</div>
        </div>
        <div style={{display:"flex",flexDirection:"column",gap:8}}>
          {volumes.map(v => (
            <div
              key={v.id}
              onClick={() => onSelect(v.id)}
              style={{
                display:"grid",gridTemplateColumns:"auto 1fr auto",gap:14,alignItems:"center",
                padding:"12px 14px",
                background: selectedId === v.id ? "var(--accent-soft)" : "var(--bg-0)",
                border:".5px solid " + (selectedId === v.id ? "var(--accent)" : "var(--line-strong)"),
                borderRadius:"var(--radius)",cursor:"default",
                transition:"background .12s, border-color .12s",
              }}
            >
              <div style={{
                width:32,height:32,borderRadius:8,background:"var(--accent-soft)",
                display:"grid",placeItems:"center",color:"var(--accent-2)"
              }}><Icon.LockOpen/></div>
              <div style={{minWidth:0}}>
                <div style={{fontFamily:"var(--mono)",fontSize:13,fontWeight:600,color:"var(--fg-0)",marginBottom:2,overflow:"hidden",textOverflow:"ellipsis",whiteSpace:"nowrap"}}>{v.name}</div>
                <div style={{fontFamily:"var(--mono)",fontSize:11,color:"var(--fg-3)",overflow:"hidden",textOverflow:"ellipsis",whiteSpace:"nowrap"}}>
                  {v.source} <span style={{opacity:.5}}>→</span> {v.mount}
                </div>
              </div>
              <div style={{
                display:"flex",alignItems:"center",gap:8,
                fontFamily:"var(--mono)",fontSize:10.5,color:"var(--fg-2)",
                textTransform:"uppercase",letterSpacing:".08em"
              }}>
                <span style={{
                  color: v.readonly ? "var(--warn)" : "var(--ok)",
                  display:"inline-flex",alignItems:"center",gap:5
                }}>
                  <span style={{width:5,height:5,borderRadius:999,background:"currentColor"}}/>
                  {v.readonly ? "RO" : "RW"}
                </span>
              </div>
            </div>
          ))}
        </div>
      </div>

      {sel && (
        <div className="card-section">
          <div className="sec-head">
            <div className="sec-title">Confirm</div>
            <div className="sec-hint">flush writes and lock</div>
          </div>
          <div style={{
            background:"var(--bg-0)",border:".5px solid var(--line-strong)",borderRadius:"var(--radius)",
            padding:"var(--s4)"
          }}>
            <dl className="kv" style={{gridTemplateColumns:"auto 1fr",rowGap:4}}>
              <dt>volume</dt><dd>{sel.name}</dd>
              <dt>source</dt><dd>{sel.source}</dd>
              <dt>mount</dt><dd>{sel.mount}</dd>
              <dt>mode</dt><dd>{sel.readonly ? "read-only" : "read/write"}</dd>
            </dl>
          </div>
        </div>
      )}

      <div className="card-foot">
        <div className="foot-hint"></div>
        <div className="foot-actions">
          <button className="btn btn-primary" disabled={!sel} onClick={() => sel && onUnmount(sel.id)}>
            <Icon.Eject/> Unmount &amp; lock <span className="btn-kbd">⌘⏎</span>
          </button>
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { CreateView, MountView, UnmountView, Field, Segmented, PassphraseInput, Toggle, AuthBlock, KeyFilePicker });

/* ─── MANAGE (mounted volumes + mount form) ───────────────────── */

function ManageView({ backend, volumes, onMount, onUnmount }) {
  const [selectedId, setSelectedId] = useState(null);
  const sel = volumes.find(v => v.id === selectedId) || null;

  // Clear selection if the selected volume disappears (was unmounted)
  useEffect(() => {
    if (selectedId && !volumes.find(v => v.id === selectedId)) setSelectedId(null);
  }, [volumes, selectedId]);

  return (
    <>
      {/* ─── Mounted volumes ───────────────────────────────── */}
      <div className="card" style={{marginBottom:"var(--s5)"}}>
        <div className="card-section">
          <div className="sec-head">
            <div className="sec-title">Mounted volumes</div>
            <div className="sec-hint">{volumes.length} {volumes.length === 1 ? "volume" : "volumes"} · click to manage</div>
          </div>

          {volumes.length === 0 ? (
            <div style={{
              padding:"var(--s5) var(--s4)",textAlign:"center",
              border:".5px dashed var(--line-strong)",borderRadius:"var(--radius)",
              background:"var(--bg-0)"
            }}>
              <div style={{
                width:40,height:40,borderRadius:10,margin:"0 auto var(--s3)",
                background:"var(--bg-2)",display:"grid",placeItems:"center",color:"var(--fg-3)"
              }}>
                <Icon.Lock/>
              </div>
              <div style={{fontSize:13,fontWeight:600,color:"var(--fg-1)",marginBottom:4}}>Nothing mounted yet</div>
              <div style={{fontSize:11.5,color:"var(--fg-3)",lineHeight:1.5}}>
                Unlock a container below to mount it here.
              </div>
            </div>
          ) : (
            <div style={{display:"flex",flexDirection:"column",gap:8}}>
              {volumes.map(v => {
                const isSel = selectedId === v.id;
                return (
                  <div key={v.id} style={{
                    border:".5px solid " + (isSel ? "var(--accent)" : "var(--line-strong)"),
                    background: isSel ? "var(--accent-soft)" : "var(--bg-0)",
                    borderRadius:"var(--radius)",
                    transition:"background .12s, border-color .12s",
                    overflow:"hidden",
                  }}>
                    <div
                      onClick={() => setSelectedId(isSel ? null : v.id)}
                      style={{
                        display:"grid",gridTemplateColumns:"auto 1fr auto",gap:14,alignItems:"center",
                        padding:"12px 14px",cursor:"default",
                      }}
                    >
                      <div style={{
                        width:32,height:32,borderRadius:8,background:"var(--accent-soft)",
                        display:"grid",placeItems:"center",color:"var(--accent-2)"
                      }}><Icon.LockOpen/></div>
                      <div style={{minWidth:0}}>
                        <div style={{fontFamily:"var(--mono)",fontSize:13,fontWeight:600,color:"var(--fg-0)",marginBottom:2,overflow:"hidden",textOverflow:"ellipsis",whiteSpace:"nowrap"}}>{v.name}</div>
                        <div style={{fontFamily:"var(--mono)",fontSize:11,color:"var(--fg-3)",overflow:"hidden",textOverflow:"ellipsis",whiteSpace:"nowrap"}}>
                          {v.source} <span style={{opacity:.5}}>→</span> {v.mount}
                        </div>
                      </div>
                      <div style={{
                        display:"flex",alignItems:"center",gap:10,
                        fontFamily:"var(--mono)",fontSize:10.5,color:"var(--fg-2)",
                        textTransform:"uppercase",letterSpacing:".08em"
                      }}>
                        <span style={{
                          color: v.readonly ? "var(--warn)" : "var(--ok)",
                          display:"inline-flex",alignItems:"center",gap:5
                        }}>
                          <span style={{width:5,height:5,borderRadius:999,background:"currentColor"}}/>
                          {v.readonly ? "RO" : "RW"}
                        </span>
                        <span style={{opacity:.5,transform: isSel ? "rotate(90deg)" : "none",transition:"transform .15s",display:"inline-flex"}}>
                          <Icon.Chevron/>
                        </span>
                      </div>
                    </div>

                    {isSel && (
                      <div style={{
                        borderTop:".5px solid var(--line)",
                        padding:"var(--s4)",
                        background:"var(--bg-0)",
                        display:"flex",alignItems:"center",justifyContent:"space-between",gap:"var(--s4)",flexWrap:"wrap"
                      }}>
                        <dl className="kv" style={{gridTemplateColumns:"auto 1fr",rowGap:4,margin:0,flex:1,minWidth:220}}>
                          {v.cipher && <><dt>cipher</dt><dd>{v.cipher}</dd></>}
                          <dt>mode</dt><dd>{v.readonly ? "read-only" : "read/write"}</dd>
                        </dl>
                        <button
                          className="btn btn-primary"
                          onClick={() => onUnmount(v.id)}
                        >
                          <Icon.Eject/> Unmount &amp; lock
                        </button>
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>

      {/* ─── Mount new volume (reuse MountView) ───────────── */}
      <div style={{
        display:"flex",alignItems:"center",gap:"var(--s3)",
        margin:"0 0 var(--s4)",color:"var(--fg-2)",
        fontSize:11,textTransform:"uppercase",letterSpacing:".12em",fontWeight:600
      }}>
        <span style={{flex:1,height:1,background:"var(--line)"}}/>
        <span>Mount a volume</span>
        <span style={{flex:1,height:1,background:"var(--line)"}}/>
      </div>

      <MountView backend={backend} onMount={onMount}/>
    </>
  );
}

Object.assign(window, { ManageView });

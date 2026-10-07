// 詩織Ver3.7: Claude Codeのフックから呼ばれ、セッションの状況をSSD上のJSONへ書く。
// 詩織の「Claudeモニター」はこのJSONを読むだけ。詩織が止まっていても書き続ける。
//
// 使い方: node claude-report.js  (フックのJSONをstdinから受け取る)
// 保存先: このファイルの1つ上の data/claude-status/<session_id>.json
//
// 方針:
// - 保存するのは許可した項目だけ。プロンプト・応答の中身(prompt, last_assistant_message
//   など)は入力に含まれていても保存しない。
// - 何があっても正常終了(exit 0)する。Claudeの作業を止めない。
// - 書き込みは一時ファイル→リネーム。詩織が読んでいる最中に中途半端なファイルを見せない。
const fs = require('fs');
const os = require('os');
const path = require('path');
const { execFileSync } = require('child_process');

const STATUS_DIR = path.join(__dirname, '..', 'data', 'claude-status');
const HEARTBEAT_MIN_INTERVAL_MS = 2000; // PostToolUseの書き込みを間引く間隔
const STOP_GRACE_MS = 5000; // Stop直後に遅れて届くPostToolUseで「作業中」へ戻さない猶予
const PRUNE_ENDED_AFTER_MS = 60 * 60 * 1000; // 終了したセッションのファイルを消すまで
const PRUNE_ANY_AFTER_MS = 24 * 60 * 60 * 1000; // 更新が止まったままのファイルを消すまで

// フックのイベント名 → 状態。PostToolUseは状態を変えず、更新時刻だけ進める(生存確認)。
const EVENT_STATE = {
  SessionStart: 'idle',
  UserPromptSubmit: 'working',
  Notification: 'waiting',
  Stop: 'idle',
  SessionEnd: 'ended',
};

function hostName() {
  return os.platform() === 'win32' ? 'win' : os.platform() === 'darwin' ? 'mac' : os.platform();
}

// このフックを起動したClaude Codeプロセスのpidを、親をたどって探す。
// 詩織が「その後Claudeが生きているか」を判定するために使う(フックは生存確認の
// 通信をしないため、長時間ツールが動いている間も無音になる。時間では死活を判定できない)。
// Mac/Linuxは`ps`、Windowsは`Get-CimInstance`で、1回だけ呼ぶ。見つからなければnull。
function isClaudeName(name) {
  const base = path.basename(String(name)).toLowerCase().replace(/\.exe$/, '');
  return base === 'claude';
}

function findClaudePid() {
  try {
    if (os.platform() === 'win32') {
      const json = execFileSync(
        'powershell',
        ['-NoProfile', '-Command', 'Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name | ConvertTo-Json -Compress'],
        { timeout: 8000, encoding: 'utf8' },
      );
      const table = new Map(JSON.parse(json).map((p) => [p.ProcessId, p]));
      let pid = process.ppid;
      for (let i = 0; i < 10 && pid > 4; i++) {
        const p = table.get(pid);
        if (!p) return null;
        if (isClaudeName(p.Name)) return pid;
        pid = p.ParentProcessId;
      }
      return null;
    }
    let pid = process.ppid;
    for (let i = 0; i < 10 && pid > 1; i++) {
      const out = execFileSync('ps', ['-o', 'ppid=,comm=', '-p', String(pid)], { timeout: 3000, encoding: 'utf8' }).trim();
      const m = out.match(/^(\d+)\s+(.*)$/);
      if (!m) return null;
      if (isClaudeName(m[2])) return pid;
      pid = Number(m[1]);
    }
  } catch {
    // 取れなくても報告自体は続ける(詩織は時間による判定にフォールバックする)
  }
  return null;
}

function projectName(input) {
  const dir = process.env.CLAUDE_PROJECT_DIR;
  return dir ? path.basename(dir) : '';
}

function readJsonOrNull(file) {
  try {
    return JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch {
    return null;
  }
}

function writeAtomic(file, obj) {
  const tmp = `${file}.${process.pid}.tmp`;
  fs.writeFileSync(tmp, JSON.stringify(obj));
  fs.renameSync(tmp, file);
}

// 古いファイルの掃除。ended→1時間、更新が止まったまま→24時間で消す。
function prune(now) {
  for (const name of fs.readdirSync(STATUS_DIR)) {
    if (!name.endsWith('.json') || name.startsWith('._')) continue;
    const file = path.join(STATUS_DIR, name);
    const data = readJsonOrNull(file);
    if (!data || typeof data.updated_at !== 'number') continue;
    const age = now - data.updated_at;
    if ((data.state === 'ended' && age > PRUNE_ENDED_AFTER_MS) || age > PRUNE_ANY_AFTER_MS) {
      try {
        fs.unlinkSync(file);
      } catch {
        // 別のセッションが同時に消した場合などは無視する
      }
    }
  }
}

// ツールが動いた=作業中。入力待ちの許可が出た後や、指示を介さずに始まった作業を拾う。
// ただしStop直後は、非同期フックが遅れて届くことがあるため待機のままにする。
function heartbeatState(prev, now) {
  if (!prev) return 'working';
  if (prev.state === 'idle' && prev.last_event === 'Stop' && now - (prev.state_since ?? 0) < STOP_GRACE_MS) {
    return 'idle';
  }
  return 'working';
}

function handle(input) {
  const event = input.hook_event_name;
  const sessionId = input.session_id;
  if (typeof sessionId !== 'string' || !/^[A-Za-z0-9_-]+$/.test(sessionId)) return;
  const isHeartbeat = event === 'PostToolUse';
  if (!isHeartbeat && !(event in EVENT_STATE)) return;

  // SSDが未接続なら data/ ごと存在しない。作らずに終える(別の場所に書かない)。
  if (!fs.existsSync(path.join(__dirname, '..', 'data'))) return;
  fs.mkdirSync(STATUS_DIR, { recursive: true });

  const now = Date.now();
  const file = path.join(STATUS_DIR, `${sessionId}.json`);
  const prev = readJsonOrNull(file);

  // 終了後に遅れて届いたPostToolUse等で、endedを上書きしない。
  if (isHeartbeat && prev && prev.state === 'ended') return;
  if (isHeartbeat && prev && now - prev.updated_at < HEARTBEAT_MIN_INTERVAL_MS) return;

  // pidは、取れていればそのまま使う。取れていないときは、節目のイベント(PostToolUse以外)
  // では毎回、PostToolUseでは1回だけ探す(フックを入れた時点で動いていたセッションを拾うため)。
  const needPid = prev?.pid == null;
  const tryFind = needPid && (!isHeartbeat || !prev?.pid_tried);
  const pid = tryFind ? findClaudePid() : prev?.pid ?? null;
  const pidTried = tryFind || Boolean(prev?.pid_tried);

  const next = {
    session_id: sessionId,
    // cwdは作業中にcdで変わるため、プロジェクトのルート(CLAUDE_PROJECT_DIR)を優先する。
    // 取れないときだけ、最初に見えた名前を固定して一覧の表示を安定させる。
    project: projectName(input) || prev?.project || (input.cwd ? path.basename(input.cwd) : ''),
    title: typeof input.session_title === 'string' ? input.session_title : prev?.title ?? '',
    host: hostName(),
    pid,
    pid_tried: pidTried,
    state: isHeartbeat ? heartbeatState(prev, now) : EVENT_STATE[event],
    last_event: isHeartbeat ? prev?.last_event ?? event : event,
    started_at: prev?.started_at ?? now,
    updated_at: now,
  };
  // 状態が変わった時刻。updated_atは生存確認のたびに進むため、Stop直後の猶予判定は別に持つ。
  next.state_since = prev && prev.state === next.state ? prev.state_since ?? now : now;
  writeAtomic(file, next);

  if (event === 'SessionStart') prune(now);
}

let buf = '';
process.stdin.on('data', (d) => (buf += d));
process.stdin.on('end', () => {
  try {
    handle(JSON.parse(buf));
  } catch {
    // 入力が壊れていても、書き込みに失敗しても、Claudeの作業は止めない
  }
  process.exit(0);
});

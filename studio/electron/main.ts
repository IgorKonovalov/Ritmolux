/**
 * The main process: resolve the player, open the window, move the two pipes.
 *
 * Ordering is the thing to hold on to. The player greets and declares its
 * stream geometry before a renderer exists to hear either, so events are held
 * and replayed once the window has loaded — otherwise the panel would never see
 * `hello` and the canvas would never learn its size. Frames are not held: a
 * frame with nothing painting it is dropped and counted, which is what the
 * footer's number means.
 */
import type { BrowserWindow, MessagePortMain } from 'electron'
import { app, dialog, MessageChannelMain } from 'electron'
import { accessSync, constants, statSync } from 'node:fs'
import { join } from 'node:path'

import { IPC_CHANNELS } from '@shared/ipc-channels'
import type { PlayerEvent } from '@shared/protocol'

import { registerAppHandlers, type AppInfo } from './ipc/appHandlers'
import { registerJudgingHandlers } from './ipc/judgingHandlers'
import { registerPlayerHandlers } from './ipc/playerHandlers'
import { JudgingService } from './judging/session'
import { registerPresetHandlers, type PresetScope } from './ipc/presetHandlers'
import { registerRenderHandlers, renderEmitter, stayAwake } from './ipc/renderHandlers'
import { RenderService } from './render/service'
import { TranscodeCache } from './render/transcode'
import { SchemaCache } from './player/schema'
import { ControlSender } from './player/control'
import { judgingArgs, playerArgs, PlayerSupervisor } from './player/supervisor'
import { resolvePlayer, type ResolvedPlayer } from './player/resolve'
import type { PlayerMode } from '@shared/player-mode'

import {
  ffmpegOf,
  outputDirOf,
  playerModeOf,
  readSettings,
  reducedMotionOf,
  settingsFile,
  sourceDirOf,
  withJudgingSource,
  withRender,
  writeSettings,
  type StudioSettings,
} from './settings'
import { createWindow, DEV_SERVER_ORIGIN, getRendererPaths, installCsp } from './window'
import { captureRequest, runCapture } from './capture'

const isDev = process.env.ELECTRON_RENDERER_URL !== undefined

let supervisor: PlayerSupervisor | undefined
/** The renderer's end of the frame channel, kept so a restarted player's pump can attach to it. */
let framePort: MessagePortMain | undefined
let judging: JudgingService | undefined
let render: { service: RenderService; cache: TranscodeCache } | undefined
const control = new ControlSender()
let resolved: ResolvedPlayer | undefined
/** Events seen before the renderer loaded, replayed to it in arrival order. */
let replay: PlayerEvent[] = []
let rendererReady = false
/**
 * The paths the player has named, which are the only ones main will open.
 *
 * Kept from the events rather than resolved here: the player is the one that
 * knows where it is reading presets from, and a second resolver in the studio
 * is the thing ADR-0184 refuses.
 */
const scope: PresetScope = { file: undefined, dir: undefined }

function isExecutableFile(candidate: string): boolean {
  try {
    if (!statSync(candidate).isFile()) return false
    accessSync(candidate, constants.X_OK)
    return true
  } catch {
    return false
  }
}

function send(window: BrowserWindow, event: PlayerEvent): void {
  if (!rendererReady) {
    replay.push(event)
    return
  }
  if (!window.isDestroyed()) window.webContents.send(IPC_CHANNELS.PLAYER_EVENT, event)
}

/**
 * Stop the running player, if any, and spawn one on `args`.
 *
 * What the old player named is forgotten first: its control address, its
 * preset and its directory belong to a process that is gone, and acting on
 * them would aim the next action, or the next write, at the wrong place. Its
 * pipes can still deliver a line after the kill, so an event from any
 * supervisor but the current one is dropped.
 */
function spawnPlayer(
  window: BrowserWindow,
  playerPath: string,
  args: readonly string[],
  env?: Readonly<Record<string, string>>,
): void {
  supervisor?.stop()
  control.aim(null)
  scope.file = undefined
  scope.dir = undefined
  const self: PlayerSupervisor = new PlayerSupervisor({
    playerPath,
    args,
    env,
    onEvent: (event) => {
      if (supervisor !== self) return
      // The sender is aimed from the player's own answer, which is `null` when
      // it opened no listener.
      if (event.ev === 'hello') {
        control.aim(event.control)
        // A judging session steps only on the owner's keys (ADR-0267).
        if (judging?.session != null && event.control !== null) {
          control.send({ kind: 'transport', verb: 'hold' })
        }
      }
      if (event.ev === 'preset') scope.file = event.file ?? undefined
      if (event.ev === 'roster') scope.dir = event.dir ?? undefined
      if (event.ev === 'marks') judging?.marks(event)
      send(window, event)
    },
    onDiagnostic: (line) => console.log(`[player] ${line}`),
    onMalformed: (line, reason) => console.warn(`[player] unreadable event (${reason}): ${line}`),
    onRefused: (reason) => console.error(`[player] refused: ${reason}`),
    onExit: (code, signal) => console.error(`[player] exited: code=${code} signal=${signal}`),
  })
  supervisor = self
  if (framePort !== undefined) self.pump.attach(framePort)
  self.start()
}

function start(): void {
  const file = settingsFile(app.getPath('userData'))
  // The file as this session last wrote it: every setter merges onto this and
  // then replaces it, so a second write in one session keeps the first.
  let settings = readSettings(file)
  // Read once, at spawn. A mode changed later is written to the file and picked
  // up by the next launch: switching live would mean tearing down the player,
  // the control socket and the frame port under whatever is unsaved (ADR-0186
  // Alternative A).
  const mode = playerModeOf(settings)
  resolved = resolvePlayer({
    resourcesPath: app.isPackaged ? process.resourcesPath : undefined,
    settingsPath: settings.playerPath,
    pathEnv: process.env.PATH,
    exists: isExecutableFile,
  })

  const info = (): AppInfo => ({
    studioVersion: app.getVersion(),
    playerPath: resolved?.path,
    playerSource: resolved?.source,
    settingsFile: file,
    playerMode: mode,
    reducedMotion: reducedMotionOf(settings),
    render: settings.render ?? {},
  })
  const schema = new SchemaCache(resolved?.path)
  // Merged onto what was read, because `writeSettings` writes what it is handed
  // and `playerPath` is the key nobody would notice losing until a relaunch.
  // `settings` is replaced only after the write succeeded, so a refused write
  // leaves the session agreeing with the file.
  const update = (next: StudioSettings): void => {
    writeSettings(file, next)
    settings = next
  }
  const setPlayerMode = (next: PlayerMode): void => update({ ...settings, playerMode: next })
  const setReducedMotion = (next: boolean): void =>
    update({ ...settings, ui: { ...settings.ui, reducedMotion: next } })
  registerAppHandlers(info, () => schema.get(), setPlayerMode, setReducedMotion)
  registerPresetHandlers(() => scope)
  registerPlayerHandlers(
    () => control,
    (reason) => console.warn(`[studio] refused an action from the renderer: ${reason}`),
  )

  // The settings are read per call, so a path changed in the panel applies to
  // the next render without a relaunch.
  let shown: BrowserWindow | undefined
  const cache = new TranscodeCache(join(app.getPath('userData'), 'render-cache'), () =>
    ffmpegOf(settings),
  )
  const service = new RenderService({
    player: () => resolved?.path,
    ffmpeg: () => ffmpegOf(settings),
    diffusion: () => settings.render?.diffusion,
    outputDir: () => outputDirOf(settings, app.getPath('videos')),
    cache,
    emit: renderEmitter(() => shown),
    stayAwake,
  })
  render = { service, cache }
  registerRenderHandlers(service, () => shown, (next) => update(withRender(settings, next)))

  // A session takes over the one player (ADR-0183): Start restarts it on the
  // session's copy and End on the vector this launch chose.
  judging = new JudgingService({
    root: join(app.getPath('userData'), 'judging'),
    sourceDir: () => sourceDirOf(settings),
    restartPlayer: (launch) => {
      const player = resolved?.path
      if (player === undefined || shown === undefined) throw new Error('no player was found')
      if (launch === undefined) spawnPlayer(shown, player, playerArgs(mode))
      else {
        spawnPlayer(shown, player, judgingArgs(launch.marks), { RLX_PRESET_DIR: launch.presetDir })
      }
    },
  })
  registerJudgingHandlers({
    service: judging,
    window: () => shown,
    sourceDir: () => sourceDirOf(settings),
    setSourceDir: (next) => update(withJudgingSource(settings, next)),
  })

  installCsp(isDev)
  const { rendererFile, preloadPath } = getRendererPaths()
  const window = createWindow({
    preloadPath,
    rendererFile,
    rendererUrl: isDev ? DEV_SERVER_ORIGIN : undefined,
  })
  shown = window

  // A render is a child of this process and dies with it (ADR-0262), so closing
  // the window while one runs asks first.
  window.on('close', (event) => {
    if (!service.busy) return
    const choice = dialog.showMessageBoxSync(window, {
      type: 'warning',
      buttons: ['Keep rendering', 'Quit and lose the render'],
      defaultId: 0,
      cancelId: 0,
      message: 'A clip is still rendering.',
      detail: 'Quitting stops it and deletes the unfinished file.',
    })
    if (choice === 0) event.preventDefault()
  })

  window.webContents.on('did-finish-load', () => {
    // The port first, so a frame that arrives during the replay has somewhere
    // to go, then the held events in the order they were read.
    const channel = new MessageChannelMain()
    channel.port1.on('message', () => supervisor?.pump.ack())
    channel.port1.start()
    framePort = channel.port1
    supervisor?.pump.attach(channel.port1)
    window.webContents.postMessage(IPC_CHANNELS.PLAYER_FRAME, null, [channel.port2])

    rendererReady = true
    const held = replay
    replay = []
    for (const event of held) window.webContents.send(IPC_CHANNELS.PLAYER_EVENT, event)
  })

  const capture = captureRequest(process.argv)
  if (capture !== undefined && !app.isPackaged) void runCapture(window, capture)

  if (resolved === undefined) {
    // No player: the window still opens and says so. A dialog would leave a
    // tester with nothing to read and nowhere to put a path.
    return
  }

  spawnPlayer(window, resolved.path, playerArgs(mode))
}

app
  .whenReady()
  .then(start)
  .catch((error: unknown) => {
    console.error(error)
    app.quit()
  })

app.on('window-all-closed', () => app.quit())

// The child holds a GPU adapter and an audio capture; leaving it behind would
// keep both after the window is gone.
app.on('will-quit', () => {
  supervisor?.stop()
  control.close()
  // A render still running here was confirmed away by the close prompt.
  render?.service.abandon()
  render?.cache.clear()
})

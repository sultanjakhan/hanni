// ── body-viewer.js — Three.js 3D body model viewer ──
// Three.js loaded globally via <script> tag (window.THREE, etc.)

let scene, camera, renderer, controls, raycaster, mouse;
let clickableObjects = [];
let highlightedObject = null;
let originalMaterials = new Map();
let zoneHighlights = new Map();
let animFrameId = null;
let containerEl = null;
let modelBoundsY = { min: -1, max: 1 };

const HOVER_EMISSIVE = 0x66aaff;
const SELECT_EMISSIVE = 0x00ff88;

export function initViewer(container, { onSelect, onContextMenu }) {
  const T = window.THREE;
  if (!T) { console.error('THREE not loaded'); return; }
  containerEl = container;
  scene = new T.Scene();
  scene.background = null;

  camera = new T.PerspectiveCamera(35, container.clientWidth / container.clientHeight, 0.01, 100);
  camera.position.set(1.5, 1.5, 1.5);

  renderer = new T.WebGLRenderer({ antialias: true, alpha: true });
  renderer.setSize(container.clientWidth, container.clientHeight);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  container.appendChild(renderer.domElement);

  controls = new window.THREE_OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;
  controls.dampingFactor = 0.08;
  controls.enablePan = true;
  controls.panSpeed = 1.2;
  controls.screenSpacePanning = true;
  controls.enableZoom = false; // disable scroll zoom — we use scroll for pan
  controls.mouseButtons = {
    LEFT: window.THREE.MOUSE.ROTATE,
    MIDDLE: window.THREE.MOUSE.PAN,
    RIGHT: window.THREE.MOUSE.PAN,
  };

  // Scroll = vertical pan (move along body), clamped to model bounds
  container.addEventListener('wheel', (e) => {
    e.preventDefault();
    const panStep = 0.025;
    const dy = e.deltaY > 0 ? -panStep : panStep;
    const newY = controls.target.y + dy;
    if (newY < modelBoundsY.min || newY > modelBoundsY.max) return;
    controls.target.y = newY;
    camera.position.y += dy;
    controls.update();
  }, { passive: false });

  // Ctrl+Plus/Minus = zoom
  container.addEventListener('keydown', (e) => {
    if (!e.ctrlKey && !e.metaKey) return;
    const zoomStep = 0.15;
    if (e.key === '=' || e.key === '+') {
      e.preventDefault();
      camera.position.lerp(controls.target, zoomStep);
      controls.update();
    } else if (e.key === '-') {
      e.preventDefault();
      const dir = camera.position.clone().sub(controls.target).normalize();
      camera.position.add(dir.multiplyScalar(zoomStep));
      controls.update();
    }
  });
  container.tabIndex = 0; // make focusable for keyboard events

  scene.add(new T.AmbientLight(0xffffff, 0.7));
  const dir1 = new T.DirectionalLight(0xffffff, 0.8);
  dir1.position.set(5, 8, 5);
  scene.add(dir1);
  const dir2 = new T.DirectionalLight(0xffffff, 0.3);
  dir2.position.set(-5, -3, -5);
  scene.add(dir2);

  raycaster = new T.Raycaster();
  mouse = new T.Vector2();

  loadModel(T);
  setupEvents(container, onSelect, onContextMenu);
  animate();

  const ro = new ResizeObserver(() => onResize(container));
  ro.observe(container);
  container._bodyResizeObserver = ro;
}

function loadModel(T) {
  const loader = new window.THREE_GLTFLoader();
  const draco = new window.THREE_DRACOLoader();
  draco.setDecoderPath('./assets/draco/');
  loader.setDRACOLoader(draco);

  loader.load('./assets/body.glb', (gltf) => {
    const model = gltf.scene;
    model.scale.set(3, 3, 3);
    scene.add(model);

    const box = new T.Box3().setFromObject(model);
    const center = box.getCenter(new T.Vector3());
    model.position.sub(center);

    // Recalculate bounds after centering
    const box2 = new T.Box3().setFromObject(model);
    const size = box2.getSize(new T.Vector3());
    modelBoundsY = { min: box2.min.y, max: box2.max.y };

    const vFov = camera.fov * (Math.PI / 180);
    const dist = (size.y / 2) / Math.tan(vFov / 2) * 0.65;
    camera.position.set(0, 0, dist);
    controls.target.set(0, 0, 0);
    controls.update();

    model.traverse((child) => {
      if (!child.isMesh) return;
      const type = child.userData?.type;
      if (type && type !== 'bone') { child.visible = false; return; }
      clickableObjects.push(child);
      originalMaterials.set(child.uuid, child.material);
    });

    applyZoneHighlights();
  }, null, (err) => console.error('GLB load error:', err));
}

function setupEvents(container, onSelect, onContextMenu) {
  let hoveredObj = null;
  let rightDownPos = null;

  container.addEventListener('mousedown', (e) => {
    if (e.button === 2) rightDownPos = { x: e.clientX, y: e.clientY };
  });

  container.addEventListener('mousemove', (e) => {
    const rect = renderer.domElement.getBoundingClientRect();
    mouse.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
    mouse.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;
    raycaster.setFromCamera(mouse, camera);
    const hits = raycaster.intersectObjects(clickableObjects, false);
    if (hits.length > 0) {
      const obj = hits[0].object;
      if (hoveredObj !== obj) {
        if (hoveredObj && hoveredObj !== highlightedObject) restoreMaterial(hoveredObj);
        hoveredObj = obj;
        if (obj !== highlightedObject) setEmissive(obj, HOVER_EMISSIVE, 0.35);
        renderer.domElement.style.cursor = 'pointer';
      }
    } else {
      if (hoveredObj && hoveredObj !== highlightedObject) restoreMaterial(hoveredObj);
      hoveredObj = null;
      renderer.domElement.style.cursor = 'grab';
    }
  });

  container.addEventListener('click', () => {
    raycaster.setFromCamera(mouse, camera);
    const hits = raycaster.intersectObjects(clickableObjects, false);
    if (hits.length > 0) { selectObject(hits[0].object); onSelect?.(hits[0].object); }
    else { deselectAll(); onSelect?.(null); }
  });

  container.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    // Only show menu on short click (not drag/pan)
    if (rightDownPos) {
      const dx = Math.abs(e.clientX - rightDownPos.x);
      const dy = Math.abs(e.clientY - rightDownPos.y);
      if (dx > 5 || dy > 5) { rightDownPos = null; return; }
    }
    rightDownPos = null;
    const rect = renderer.domElement.getBoundingClientRect();
    mouse.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
    mouse.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;
    raycaster.setFromCamera(mouse, camera);
    const hits = raycaster.intersectObjects(clickableObjects, false);
    if (hits.length > 0) {
      selectObject(hits[0].object);
      onContextMenu?.(e.clientX, e.clientY, hits[0].object);
    }
  });
}

function selectObject(obj) {
  if (highlightedObject && highlightedObject !== obj) restoreMaterial(highlightedObject);
  highlightedObject = obj;
  setEmissive(obj, SELECT_EMISSIVE, 0.6);
}

function deselectAll() {
  if (highlightedObject) { restoreMaterial(highlightedObject); highlightedObject = null; }
}

function setEmissive(obj, color, intensity) {
  const T = window.THREE;
  if (!obj._bodyCloned) { obj.material = obj.material.clone(); obj._bodyCloned = true; }
  obj.material.emissive = new T.Color(color);
  obj.material.emissiveIntensity = intensity;
}

function restoreMaterial(obj) {
  const orig = originalMaterials.get(obj.uuid);
  if (orig) { obj.material = orig; obj._bodyCloned = false; }
}

export function setZoneHighlights(highlights) {
  zoneHighlights = highlights;
  applyZoneHighlights();
}

function applyZoneHighlights() {
  clickableObjects.forEach(obj => {
    const zoneName = obj.userData?.name || obj.name;
    const color = zoneHighlights.get(zoneName);
    if (color && obj !== highlightedObject) setEmissive(obj, color, 0.25);
  });
}

function onResize(container) {
  if (!camera || !renderer) return;
  camera.aspect = container.clientWidth / container.clientHeight;
  camera.updateProjectionMatrix();
  renderer.setSize(container.clientWidth, container.clientHeight);
}

function animate() {
  animFrameId = requestAnimationFrame(animate);
  controls?.update();
  renderer?.render(scene, camera);
}

export function disposeViewer() {
  if (animFrameId) cancelAnimationFrame(animFrameId);
  animFrameId = null;
  if (containerEl?._bodyResizeObserver) {
    containerEl._bodyResizeObserver.disconnect();
    delete containerEl._bodyResizeObserver;
  }
  renderer?.dispose();
  controls?.dispose();
  scene?.traverse(obj => {
    if (obj.isMesh) { obj.geometry?.dispose(); obj.material?.dispose?.(); }
  });
  clickableObjects = [];
  originalMaterials.clear();
  zoneHighlights.clear();
  highlightedObject = null;
  if (containerEl) containerEl.innerHTML = '';
}

export function zoomCamera(factor) {
  if (!controls || !camera) return;
  const dir = camera.position.clone().sub(controls.target);
  dir.multiplyScalar(factor);
  camera.position.copy(controls.target).add(dir);
  controls.update();
}

export function panCamera(dx, dy) {
  if (!controls || !camera) return;
  const newY = Math.max(modelBoundsY.min, Math.min(modelBoundsY.max, controls.target.y + dy));
  const clampedDy = newY - controls.target.y;
  controls.target.y = newY;
  camera.position.y += clampedDy;
  controls.target.x += dx;
  camera.position.x += dx;
  controls.update();
}

export function resetCamera() {
  if (!controls || !camera) return;
  controls.target.set(0, 0, 0);
  const T = window.THREE;
  const dist = 1.5;
  camera.position.set(dist * 0.6, dist * 0.3, dist * 0.6);
  controls.update();
}

export function getZoneInfo(obj) {
  return {
    zone: obj.name || 'unknown',
    label: obj.userData?.name || obj.userData?.nameDetail || obj.name || 'Unknown',
    type: obj.userData?.type || 'bone',
  };
}

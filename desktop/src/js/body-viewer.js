// ── body-viewer.js — Three.js 3D body model viewer ──
// Three.js loaded globally via <script> tag (window.THREE, etc.)

let scene, camera, renderer, controls, raycaster, mouse;
let clickableObjects = [];
let highlightedObject = null;
let originalMaterials = new Map();
let zoneHighlights = new Map();
let animFrameId = null;
let containerEl = null;

const HOVER_EMISSIVE = 0x4488ff;
const SELECT_EMISSIVE = 0x2266cc;

export function initViewer(container, { onSelect, onContextMenu }) {
  const T = window.THREE;
  if (!T) { console.error('THREE not loaded'); return; }
  containerEl = container;
  scene = new T.Scene();
  scene.background = null;

  camera = new T.PerspectiveCamera(60, container.clientWidth / container.clientHeight, 0.01, 100);
  camera.position.set(1.5, 1.5, 1.5);

  renderer = new T.WebGLRenderer({ antialias: true, alpha: true });
  renderer.setSize(container.clientWidth, container.clientHeight);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  container.appendChild(renderer.domElement);

  controls = new window.THREE_OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;
  controls.dampingFactor = 0.08;
  controls.minDistance = 0.3;
  controls.maxDistance = 10;

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
    scene.add(model);

    const box = new T.Box3().setFromObject(model);
    const center = box.getCenter(new T.Vector3());
    model.position.sub(center);

    const size = box.getSize(new T.Vector3());
    const maxDim = Math.max(size.x, size.y, size.z);
    const dist = maxDim * 1.2;
    camera.position.set(dist * 0.6, dist * 0.4, dist * 0.6);
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
        if (obj !== highlightedObject) setEmissive(obj, HOVER_EMISSIVE, 0.15);
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
  setEmissive(obj, SELECT_EMISSIVE, 0.3);
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

export function getZoneInfo(obj) {
  return {
    zone: obj.name || 'unknown',
    label: obj.userData?.name || obj.userData?.nameDetail || obj.name || 'Unknown',
    type: obj.userData?.type || 'bone',
  };
}

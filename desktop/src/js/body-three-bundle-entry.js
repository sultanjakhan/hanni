// Entry point for esbuild bundling — exposes Three.js + addons as globals
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
import { DRACOLoader } from 'three/examples/jsm/loaders/DRACOLoader.js';

window.THREE = THREE;
window.THREE_OrbitControls = OrbitControls;
window.THREE_GLTFLoader = GLTFLoader;
window.THREE_DRACOLoader = DRACOLoader;

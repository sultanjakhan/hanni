// recipe-shared-image.js — Client-side image downscale for recipe photos.
// Loaded as a plain <script> BEFORE recipe-shared.js. Registers
// window.HanniRecipe.image.compress(file) → Promise<dataURL>. The recipe photo
// is stored as a JPEG data URL in the DB (no Rust file handling needed).
(function () {
  function compress(file, max = 900, quality = 0.8) {
    return new Promise((resolve, reject) => {
      const url = URL.createObjectURL(file);
      const img = new Image();
      img.onload = () => {
        URL.revokeObjectURL(url);
        let w = img.naturalWidth, h = img.naturalHeight;
        if (w > max || h > max) { const r = Math.min(max / w, max / h); w = Math.round(w * r); h = Math.round(h * r); }
        const canvas = document.createElement('canvas');
        canvas.width = w; canvas.height = h;
        canvas.getContext('2d').drawImage(img, 0, 0, w, h);
        try { resolve(canvas.toDataURL('image/jpeg', quality)); }
        catch (e) { reject(e); }
      };
      img.onerror = () => { URL.revokeObjectURL(url); reject(new Error('image load failed')); };
      img.src = url;
    });
  }

  // Photo field markup for the wizard's step 1 (preview + pick button).
  function fieldHtml(r) {
    const img = (r && r.image) || '';
    return `<div class="form-group rw-photo">
      <img class="rw-photo-img"${img ? '' : ' style="display:none"'} src="${img}">
      <label class="btn-secondary rw-photo-btn">📷 Фото<input type="file" accept="image/*" id="r-photo" hidden></label>
    </div>`;
  }

  // Wire the file input: compress on pick, update preview + state.image.
  function attach(overlay, state) {
    const inp = overlay.querySelector('#r-photo');
    if (!inp) return;
    inp.onchange = async () => {
      const f = inp.files && inp.files[0];
      if (!f) return;
      try {
        state.image = await compress(f);
        const im = overlay.querySelector('.rw-photo-img');
        if (im) { im.src = state.image; im.style.display = ''; }
      } catch { /* ignore bad image */ }
    };
  }

  window.HanniRecipe = window.HanniRecipe || {};
  window.HanniRecipe.image = { compress, fieldHtml, attach };
})();

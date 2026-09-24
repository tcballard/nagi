// Executed only in Nagi's isolated script world. Page text is untrusted data.
(() => {
  const request = __NAGI_REQUEST__;
  const visible = e => e.isConnected && e.getClientRects().length && getComputedStyle(e).visibility !== 'hidden';
  const sensitive = e => e.matches('input[type=password],input[type=file]') || /password|cc-|one-time-code/.test(e.autocomplete || '');
  if (request.method === 'snapshot') {
    const token = crypto.randomUUID();
    const refs = new Map();
    const nodes = [];
    for (const e of document.querySelectorAll('a,button,input,textarea,select,[role=button],[role=link]')) {
      if (nodes.length >= 500) break;
      if (!visible(e)) continue;
      const ref = token + ':' + nodes.length;
      refs.set(ref, e);
      nodes.push({ref, tag:e.tagName.toLowerCase(), role:e.getAttribute('role'),
        label:(e.getAttribute('aria-label') || e.innerText || e.getAttribute('placeholder') || '').slice(0,300),
        type:e.getAttribute('type'), sensitive:sensitive(e), disabled:!!e.disabled});
    }
    window.__nagiControl = {token, refs};
    return JSON.stringify({untrusted:true, token, title:document.title, url:location.href,
      text:(document.body?.innerText || '').slice(0,20000), elements:nodes,
      limitations:['top-level document only','closed shadow roots and cross-origin frames are not exposed','synthetic actions may be rejected by sites']});
  }
  if (request.method === 'scroll') {
    window.scrollBy({left:request.params.x || 0,top:request.params.y || 0,behavior:'instant'});
    return JSON.stringify({x:scrollX,y:scrollY});
  }
  const e = window.__nagiControl?.refs.get(request.params.ref);
  if (!e || !visible(e)) throw Error('Stale element reference; take a new snapshot');
  if (sensitive(e)) throw Error('Sensitive fields require manual input');
  if (e.disabled) throw Error('Element is disabled');
  if (request.method === 'click') {
    e.click();
  } else if (request.method === 'type') {
    if (!e.matches('input,textarea') || e.readOnly) throw Error('Element is not editable');
    const proto = e.tagName === 'TEXTAREA' ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(proto,'value').set.call(e, request.params.text);
    e.dispatchEvent(new Event('input',{bubbles:true}));
    e.dispatchEvent(new Event('change',{bubbles:true}));
  } else if (request.method === 'select') {
    if (e.tagName !== 'SELECT' || !Array.from(e.options).some(o=>o.value===request.params.value)) throw Error('No matching option');
    e.value = request.params.value;
    e.dispatchEvent(new Event('input',{bubbles:true}));
    e.dispatchEvent(new Event('change',{bubbles:true}));
  } else throw Error('Unsupported page action');
  return JSON.stringify({performed:true});
})();

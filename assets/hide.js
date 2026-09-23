(()=>{
 if(window.__nagiPicking)return;
 window.__nagiPicking=true;
 let target=null,previous='';
 function restore(){if(target)target.style.outline=previous;target=null;}
 function move(e){restore();target=e.target;previous=target.style.outline;target.style.outline='2px solid #b4dc81';}
 function stop(){restore();document.removeEventListener('mousemove',move,true);document.removeEventListener('click',pick,true);document.removeEventListener('keydown',key,true);window.__nagiPicking=false;}
 function key(e){if(e.key==='Escape'){e.preventDefault();stop();}}
 function pick(e){e.preventDefault();e.stopImmediatePropagation();const el=e.target;stop();if(el===document.body||el===document.documentElement)return;
  const parts=[];let n=el;
  while(n&&n!==document.documentElement){if(n.id){parts.unshift('#'+CSS.escape(n.id));break;}const tag=n.localName;if(!tag)return;let i=1;for(let p=n.previousElementSibling;p;p=p.previousElementSibling)if(p.localName===tag)i++;parts.unshift(tag+':nth-of-type('+i+')');n=n.parentElement;}
  const selector=parts.join(' > ');el.style.setProperty('display','none','important');window.webkit.messageHandlers.nagiHide.postMessage(JSON.stringify({origin:location.origin,selector}));
 }
 document.addEventListener('mousemove',move,true);document.addEventListener('click',pick,true);document.addEventListener('keydown',key,true);
})();

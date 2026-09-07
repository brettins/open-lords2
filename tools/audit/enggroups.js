const fs=require('fs');const b=fs.readFileSync('F:/games/Lords of the Realm II/L2.eng');
const off=g=>b[8+g*4]|(b[9+g*4]<<8)|(b[10+g*4]<<16);
const N=(off(1)-8)/4;
const G=g=>{const s=off(g),e=(g+1<N)?off(g+1):b.length;if(e<=s)return[];const p=b.subarray(s,e).toString('latin1').split('\0');p.pop();return p;};
for(const g of [98,99,100,101,102,103,104]) {const x=G(g);console.log('group',g,'count',x.length,':',JSON.stringify(x.slice(0,14)));}
// locate 'England'
for(let g=1;g<N;g++){const x=G(g);if(x.includes('England'))console.log('England in group',g,'index',x.indexOf('England'),'count',x.length);}
for(let g=1;g<N;g++){const x=G(g);if(x.some(s=>/^map no 25$/.test(s)))console.log('"map no 25" in group',g,'index',x.indexOf('map no 25'),'count',x.length);}
for(let g=1;g<N;g++){const x=G(g);if(x.includes('Wiltshire'))console.log('Wiltshire in group',g,'count',x.length);}
for(let g=1;g<N;g++){const x=G(g);if(x.includes('New Game')||x.includes('New game'))console.log('New Game in group',g,':',JSON.stringify(x));}
for(let g=1;g<N;g++){const x=G(g);if(x.includes('Crossbows'))console.log('goods in group',g,':',JSON.stringify(x));}
for(let g=1;g<N;g++){const x=G(g);if(x.some(s=>/^Scrubland\.$/.test(s)))console.log('terrain names in group',g,':',JSON.stringify(x.slice(0,12)));}
for(let g=1;g<N;g++){const x=G(g);if(x.includes('Null tool tip'))console.log('tooltips in group',g,'count',x.length);}
// full map name group
const mg=(()=>{for(let g=1;g<N;g++){const x=G(g);if(x.includes('England')&&x.includes('Australia'))return g;}})();
console.log('\nmap-name group',mg,'->',JSON.stringify(G(mg)));

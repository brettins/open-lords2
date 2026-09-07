const fs=require('fs');
function load(p){const b=fs.readFileSync(p);const off=g=>b[8+g*4]|(b[9+g*4]<<8)|(b[10+g*4]<<16);const N=(off(1)-8)/4;
 const G=g=>{const s=off(g),e=(g+1<N)?off(g+1):b.length;return b.subarray(s,e);};return{b,off,N,G};}
const W=load('F:/games/Lords of the Realm II/L2.eng'),D=load('F:/games/LORDS2/L2.ENG');
let ident=0,diff=[];
for(let g=1;g<D.N;g++){ if(Buffer.compare(W.G(g),D.G(g))===0)ident++; else diff.push(g); }
console.log('shared ids 1..'+(D.N-1)+':','byte-identical',ident,'differing',diff.length,diff);
let empW=0,empD=0;for(let g=1;g<W.N;g++)if(W.G(g).length===0)empW++;for(let g=1;g<D.N;g++)if(D.G(g).length===0)empD++;
console.log('empty groups win',empW,'dos',empD,'group 41 empty (win):',W.G(41).length===0);
console.log('windows appended ids',D.N,'-',W.N-1,'count',W.N-D.N);

const fs=require('fs');
function l2(p){const b=fs.readFileSync(p);
 const off=g=>b[8+g*4]|(b[9+g*4]<<8)|(b[10+g*4]<<16);
 const N=(off(1)-8)/4;
 const fourth=[];for(let g=0;g<N;g++)if(b[11+g*4]!==0)fourth.push(g);
 let strings=0,empty=0,groups=[null],bad=[];  // index 0 unused: group ids start at 1
 for(let g=1;g<N;g++){const s=off(g),e=(g+1<N)?off(g+1):b.length;
  if(e===s){empty++;groups.push([]);continue;}
  if(e<s){bad.push('decreasing at '+g);groups.push([]);continue;}
  const seg=b.subarray(s,e); if(seg[seg.length-1]!==0)bad.push('group '+g+' does not end on NUL');
  const parts=seg.toString('latin1').split('\0'); parts.pop();
  strings+=parts.length; groups.push(parts);}
 let ctrl=0;for(let i=off(1);i<b.length;i++)if(b[i]<0x20&&b[i]!==0)ctrl++;
 console.log(p,'size',b.length,'magic',b.subarray(0,8).toString(),'offset(1)',off(1),'N',N,'ids 1..'+(N-1),
  'strings',strings,'empty groups',empty,'4th byte nonzero slots',fourth.length,'control bytes',ctrl,'problems',bad);
 return {groups,off,N,b};
}
const win=l2('F:/games/Lords of the Realm II/L2.eng');
const dos=l2('F:/games/LORDS2/L2.ENG');
console.log('\ngroup 101 count:',win.groups[101].length);
console.log(win.groups[101].map((s,i)=>i+':'+s).join(' | '));
console.log('\ngroup 100 count:',win.groups[100].length);
console.log('group 1:',win.groups[1]);
console.log('group 6:',win.groups[6]);
console.log('group 30:',win.groups[30]);
console.log('group 41 length:',win.groups[41].length);
// DOS vs Windows group identity
let ident=0,diff=[];
for(let g=1;g<dos.N;g++){const a=win.groups[g]||[],c=dos.groups[g]||[];if(JSON.stringify(a)===JSON.stringify(c))ident++;else diff.push(g);}
console.log('\nshared group ids',dos.N-1,'identical:',ident,'differing:',diff.length,diff.slice(0,30));
console.log('windows extra group ids:',dos.N,'..',win.N-1,'=',win.N-dos.N);
// BATTLES.ENG
{const b=fs.readFileSync('F:/games/Lords of the Realm II/BATTLES.ENG');
 const t=Buffer.from(b);for(let i=0;i<t.length;i++)if(t[i]<0x20)t[i]=0;
 const f=t.toString('latin1').split('\0').filter(s=>s.length>0);
 console.log('\nBATTLES.ENG size',b.length,'non-empty fields',f.length,'records',f.length/3);
 console.log(' first triple:',f.slice(0,3),'\n records 35,36:',f.slice(105,111),'\n last:',f.slice(-3));}
// TROOPS
for(const n of ['TROOPS.ENG','TROOPS2.ENG','TROOPS3.ENG']){
 const s=fs.readFileSync('F:/games/Lords of the Realm II/'+n,'latin1');
 const i=s.indexOf('*'); const rest=s.slice(i);
 const nums=rest.match(/\d+/g)||[]; const stars=(rest.match(/\*/g)||[]).length;
 // clamp check on cols 7-10
 const v=nums.map(Number); let over=0;
 for(let r=0;r<35;r++)for(let g=0;g<5;g++)for(let sd=0;sd<2;sd++)for(let c=0;c<11;c++){
   const idx=r*111+1+ (g*2+sd)*11 + c; if(c>=7&&v[idx]>9)over++;}
 console.log(n,'size',fs.statSync('F:/games/Lords of the Realm II/'+n).size,'numbers after first *:',nums.length,'stars:',stars,'siege cols >9:',over);}

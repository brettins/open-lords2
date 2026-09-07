const fs=require('fs');
const b=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
const REC=32961;
function prof(o,len,label){
  const h={};let z=0;for(let i=o;i<o+len;i++){h[b[i]]=(h[b[i]]||0)+1;}
  const e=Object.entries(h).sort((a,c)=>c[1]-a[1]);
  console.log(label.padEnd(22),'distinct',String(e.length).padStart(3),' top10:',e.slice(0,10).map(([v,c])=>`${(+v).toString(16)}=${(100*c/len).toFixed(1)}%`).join(' '));
}
for(const r of [0,1,11,40,50]){
  const o=r*REC;console.log('--- record',r,'---');
  prof(o,8192,' planeA [0,8192)');
  prof(o+8192,8192,' planeB [8192,16384)');
  prof(o+16384,8192,' planeC [16384,24576)');
  prof(o+24576,8385,' tail   [24576,32961)');
}
// autocorrelation within each segment for record 0
function best(o,len,maxd){const out=[];for(let d=1;d<=maxd;d++){let eq=0,n=0;for(let i=o;i+d<o+len;i++){n++;if(b[i]===b[i+d])eq++;}out.push([d,eq/n]);}out.sort((a,c)=>c[1]-a[1]);return out.slice(0,6);}
console.log('\nautocorr per segment, record 0:');
console.log(' planeA', best(0,8192,300).map(([d,s])=>d+':'+s.toFixed(3)).join(' '));
console.log(' planeB', best(8192,8192,300).map(([d,s])=>d+':'+s.toFixed(3)).join(' '));
console.log(' planeC', best(16384,8192,300).map(([d,s])=>d+':'+s.toFixed(3)).join(' '));
console.log(' tail  ', best(24576,8385,300).map(([d,s])=>d+':'+s.toFixed(3)).join(' '));

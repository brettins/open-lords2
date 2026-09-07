const fs=require('fs');const D='F:/games/Lords of the Realm II/';
const N=['TROOPS.ENG','TROOPS2.ENG','TROOPS3.ENG'];
const V={};
for(const n of N){const s=fs.readFileSync(D+n,'latin1');const v=(s.slice(s.indexOf('*')).match(/\d+/g)||[]).map(Number);V[n]=v;
 const grp=[0,0,0,0,0];
 for(let r=0;r<35;r++)for(let g=0;g<5;g++){let sum=0;for(let sd=0;sd<2;sd++)for(let c=0;c<11;c++)sum+=v[r*111+1+(g*2+sd)*11+c];grp[g]+=sum;}
 console.log(n,'per-difficulty-group value sums:',grp,'mtime',fs.statSync(D+n).mtime.toISOString().slice(0,10));}
for(let i=0;i<3;i++)for(let j=i+1;j<3;j++){const a=fs.readFileSync(D+N[i]),b=fs.readFileSync(D+N[j]);let d=0;for(let k=0;k<a.length;k++)if(a[k]!==b[k])d++;console.log(N[i],'vs',N[j],'differing bytes',d,'sizes',a.length,b.length);}

console.log('\n-- split troop cols 0-6 vs siege cols 7-10 --');
for(const n of N){const v=V[n];const a=[0,0,0,0,0],b=[0,0,0,0,0];
 for(let r=0;r<35;r++)for(let g=0;g<5;g++)for(let sd=0;sd<2;sd++)for(let c=0;c<11;c++){
  const x=v[r*111+1+(g*2+sd)*11+c]; if(c<7)a[g]+=x; else b[g]+=x;}
 console.log(n,'cols0-6 by group',a,'cols7-10 by group',b);}
console.log('\n-- first row of TROOPS2 --');
console.log(V['TROOPS2.ENG'].slice(0,111).join(' '));

console.log('\n-- rows with non-zero values outside the Normal group --');
for(const n of N){const v=V[n];const rows=[];
 for(let r=0;r<35;r++){let s=0;for(const g of [0,1,3,4])for(let sd=0;sd<2;sd++)for(let c=0;c<11;c++)s+=v[r*111+1+(g*2+sd)*11+c];
  if(s)rows.push(r);}
 console.log(n,'rows:',rows.length, rows.join(','));}
console.log('\nTROOPS2 row 34:',V['TROOPS2.ENG'].slice(34*111,35*111).join(' '));

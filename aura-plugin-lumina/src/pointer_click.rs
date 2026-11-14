//! Bounded native double/triple-click sequencing. No timers/threads, no
//! retained text, only callback ID, editor authority and physical geometry.
//! A drag, new Host epoch, blur/foreign callback, old press or malformed
//! position breaks a sequence rather than reusing stale selection authority.
#[derive(Clone,Copy,Debug)]
pub(crate) struct ClickSequence {
    pub(crate) callback_id:u64,
    pub(crate) epoch:u64,
    pub(crate) generation:u64,
    pub(crate) clicks:u8,
    pub(crate) x:f32,
    pub(crate) y:f32,
    released_at:Option<f64>,
    moved:bool,
}
const MAX_INTERVAL_SECS:f64=0.45;
const MAX_MOTION_PX:f32=6.0;
impl ClickSequence {
    pub(crate) fn begin(previous:Option<Self>,callback_id:u64,
        epoch:u64,generation:u64,x:f32,y:f32,now:f64)->Option<Self>{
        if !x.is_finite()||!y.is_finite()||!now.is_finite(){return None;}
        let clicks=previous.filter(|last|{
            last.callback_id==callback_id && last.epoch==epoch &&
            last.generation==generation && !last.moved &&
            last.released_at.is_some_and(|release|now>=release &&
                now-release<=MAX_INTERVAL_SECS) &&
            (x-last.x).abs()<=MAX_MOTION_PX &&
            (y-last.y).abs()<=MAX_MOTION_PX
        }).map(|last|(last.clicks%3)+1).unwrap_or(1);
        Some(Self{callback_id,epoch,generation,clicks,x,y,
            released_at:None,moved:false})
    }
    pub(crate) fn observe_motion(&mut self,x:f32,y:f32){
        if !x.is_finite()||!y.is_finite() ||
            (x-self.x).abs()>MAX_MOTION_PX ||
            (y-self.y).abs()>MAX_MOTION_PX {self.moved=true;}
    }
    pub(crate) fn release(&mut self,now:f64){
        self.released_at=if !self.moved&&now.is_finite(){Some(now)}
            else{None};
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn multi_click_counts_exact_three_then_restarts_without_stale_authority(){
        let mut s=ClickSequence::begin(None,7,10,3,50.0,30.0,1.0).unwrap();
        assert_eq!(s.clicks,1);
        s.release(1.01);
        s=ClickSequence::begin(Some(s),7,10,3,52.0,30.0,1.2).unwrap();
        assert_eq!(s.clicks,2);
        s.epoch=11; // The actual editor changes selection/authority on double click.
        s.release(1.21);
        s=ClickSequence::begin(Some(s),7,11,3,51.0,30.0,1.3).unwrap();
        assert_eq!(s.clicks,3);
        s.epoch=12;
        s.release(1.31);
        assert_eq!(ClickSequence::begin(Some(s),7,12,3,51.0,30.0,1.4)
            .unwrap().clicks,1);
    }
    #[test]
    fn rejects_drag_timeout_new_host_and_unrelated_callbacks(){
        let mut s=ClickSequence::begin(None,7,10,3,50.0,30.0,1.0).unwrap();
        s.observe_motion(70.0,30.0);s.release(1.02);
        assert_eq!(ClickSequence::begin(Some(s),7,10,3,50.0,30.0,1.1)
            .unwrap().clicks,1);
        let mut s=ClickSequence::begin(None,7,10,3,50.0,30.0,1.0).unwrap();
        s.release(1.01);
        for (id,epoch,generation,x,y,now) in [
            (8,10,3,50.0,30.0,1.1),
            (7,11,3,50.0,30.0,1.1),
            (7,10,4,50.0,30.0,1.1),
            (7,10,3,65.0,30.0,1.1),
            (7,10,3,50.0,30.0,1.8),
            (7,10,3,50.0,30.0,0.8)
        ]{
            assert_eq!(ClickSequence::begin(Some(s),id,epoch,generation,x,y,now)
                .unwrap().clicks,1);
        }
        assert!(ClickSequence::begin(Some(s),7,10,3,f32::NAN,30.0,1.1).is_none());
        assert!(ClickSequence::begin(Some(s),7,10,3,50.0,30.0,f64::NAN).is_none());
    }
}

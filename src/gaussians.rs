
use noisy_float::prelude::*;

#[derive(Debug, Clone, Copy)]
#[allow(unused, unreachable_code)]
pub struct Gaussian {
  pub mean: R64,
  pub sigma: R64
}

// TODO: immutables are just easier to work with
#[allow(unused, unreachable_code)]
impl Gaussian {
  pub fn add_indep(&self, other: &Gaussian) -> Gaussian {
    Gaussian {
      mean: self.mean + other.mean,
      sigma: (self.sigma*self.sigma + other.sigma*other.sigma).sqrt()
    }
  }
  
  pub fn remove_indep(&self, other: &Gaussian) -> Gaussian {
    let vari = self.sigma*self.sigma - other.sigma*other.sigma;
    if vari < -0.0001 { panic!("Negative variance"); }
    let vari = if vari < 0.0 { r64(0.0) } else { vari };
    
    Gaussian {
      mean: self.mean - other.mean,
      sigma: vari.sqrt()
    }
  }
  
  pub fn add_const(&self, c: f64) -> Gaussian {
    Gaussian {
      mean: self.mean + c,
      sigma: self.sigma
    }
  }
  
  pub fn scale(&self, s: f64) -> Gaussian {
    Gaussian {
      mean: self.mean * s,
      sigma: self.sigma * s
    }
  }
  
  pub fn shift(&self, m: f64) -> Gaussian {
    Gaussian {
      mean: self.mean + m,
      sigma: self.sigma
    }
  }
  
  pub fn restrict_above(&self, c: R64) -> Gaussian {
    use statrs::distribution::{Normal, Continuous, ContinuousCDF};
  
    let d = Normal::new(self.mean.into(), self.sigma.into()).unwrap();
    
    let zc = (c - self.mean) / self.sigma;
    let pdf_c = r64(d.pdf(c.into()));
    let sf_c = r64(d.sf(c.into()));
    
    let mu_trunc = self.mean + pdf_c / sf_c * self.sigma;
    
    let scale_fac_1: R64 = zc * pdf_c / sf_c;
    let scale_fac_2: R64 = pdf_c / sf_c;
    let scale_fac = (r64(1.0) + scale_fac_1 - scale_fac_2*scale_fac_2).sqrt();
    
    let sigma_trunc = self.sigma * scale_fac;
    
    Gaussian {
      mean: mu_trunc,
      sigma: sigma_trunc
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{r64, Gaussian};
  use statrs::distribution::{Normal, ContinuousCDF};

  #[test]
  fn test_1() {
    let d = Normal::new(1.0, 1.0).unwrap();
    let above_0 = d.sf(0.0);
    println!("above_0 = {:3}", above_0);
  }
  
  #[test]
  fn test_3() {
    let g = Gaussian { mean: r64(0.0), sigma: r64(1.0) };
    let g = g.restrict_above(r64(0.0));
    println!("{:?}", g);
  }
}


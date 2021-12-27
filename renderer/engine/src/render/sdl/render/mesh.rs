use std::f32::consts::PI;

pub fn generate_circle_mesh<const N: usize>() -> [[f32; 3]; N] {
    let half_n = N / 2;
    let mut arr = [[0.0, 0.0, 0.0]; N];

    arr[0] = [1.0, 0.0, 0.0];

    let mut xc = 1.0;
    let mut yc = 0.0;

    let div = PI / half_n as f32;
    let (sin, cos) = div.sin_cos();

    let mut i = 0;
    for _ in 1..half_n {
        let new_xc = cos * xc - sin * yc;
        yc = sin * xc + cos * yc;
        xc = new_xc;

        arr[i] = [xc, yc, 0.0];
        arr[i + 1] = [xc, -yc, 0.0];
        i += 2;
    }

    arr[N - 1] = [-1.0, 0.0, 0.0];

    arr
}

pub fn generate_quad_mesh() -> [[f32; 3]; 4] {
    [
        [-1.0, -1.0, 0.0],
        [1.0, -1.0, 0.0],
        [1.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0],
    ]
}

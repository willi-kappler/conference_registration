function hasHtml5Validation () {
  return typeof document.createElement('input').checkValidity === 'function';
}

$(function() {
    console.log("ready!");

    if (hasHtml5Validation()) {
      console.log("has html5 validation");
      $('#validate').submit(function (e) {
        console.log("submit");

        if ($("#captcha").val().trim() != "leopold2018") {
          alert("Please enter correct answer!");
          e.preventDefault();
        }

        if (!this.checkValidity()) {
          e.preventDefault();
          console.log("invalid");
          $(this).addClass('invalid');
        } else {
          console.log("valid");
          $(this).addClass('valid');
        }
      });
    }
});
